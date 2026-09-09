param()

$ErrorActionPreference = 'Stop'
$taskRepository = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$taskProduct = 'Starframe Installer Test'
$taskIdentifier = 'io.github.mastervoliumpl.starframe.installer-test'
$taskUninstallKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\$taskProduct"
$taskProductKey = "HKCU:\Software\$taskProduct"
$taskData = Join-Path $env:LOCALAPPDATA $taskIdentifier
$taskRoaming = Join-Path $env:APPDATA $taskIdentifier
$taskAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if ($taskAdmin) { throw 'Run this installer check as an ordinary user.' }
if (Get-Process -Name starframe -ErrorAction SilentlyContinue) { throw 'Close Starframe before running installer fixtures.' }
foreach ($taskExisting in @($taskUninstallKey, $taskProductKey, $taskData, $taskRoaming)) {
    if (Test-Path -LiteralPath $taskExisting) { throw 'An earlier installer fixture remains. Inspect it before retrying.' }
}

$taskEvidence = Join-Path $taskRepository ('test-results/0.6.0-packaging/install-' + [guid]::NewGuid().ToString('N'))
$taskInstall = Join-Path $taskEvidence 'application'
New-Item -ItemType Directory -Path $taskEvidence | Out-Null
$taskConfig = Join-Path $taskEvidence 'fixture.json'
$taskSentinels = @{
    'sqlite/state.db' = 'saved database fixture'
    'sqlite/legacy-backup.db' = 'retained legacy fixture'
    'library/imported.dll' = 'imported content fixture'
    'settings.json' = 'saved settings fixture'
}

function Assert-Retained {
    foreach ($taskEntry in $taskSentinels.GetEnumerator()) {
        $taskFile = Join-Path $taskData $taskEntry.Key
        if ([IO.File]::ReadAllText($taskFile) -ne $taskEntry.Value) {
            throw "Installer changed a data fixture: $($taskEntry.Key)"
        }
    }
    if ([IO.File]::ReadAllText((Join-Path $taskInstall 'unowned.txt')) -ne 'unowned fixture') {
        throw 'Installer changed an unowned installation file.'
    }
}

Push-Location $taskRepository
try {
    foreach ($taskVersion in @('0.6.0-dev.0', '0.6.0-dev.1')) {
        $taskOverride = @{
            productName = $taskProduct
            identifier = $taskIdentifier
            version = $taskVersion
            bundle = @{ publisher = $taskProduct }
        }
        [IO.File]::WriteAllText($taskConfig, ($taskOverride | ConvertTo-Json -Depth 4))
        & npm.cmd run tauri -- bundle --config src-tauri/tauri.nsis.conf.json --config $taskConfig *> (Join-Path $taskEvidence "bundle-$taskVersion.log")
        if ($LASTEXITCODE -ne 0) { throw "Fixture bundling failed for $taskVersion; inspect the retained log." }
        $taskInstaller = Join-Path $taskRepository "src-tauri/target/release/bundle/nsis/${taskProduct}_${taskVersion}_x64-setup.exe"
        if ([Diagnostics.FileVersionInfo]::GetVersionInfo($taskInstaller).ProductName -ne $taskProduct) {
            throw 'Refusing to run an installer without the isolated fixture identity.'
        }
        $taskProcess = Start-Process -FilePath $taskInstaller -ArgumentList @('/S', '/NS', "/D=$taskInstall") -WindowStyle Hidden -PassThru -Wait
        if ($taskProcess.ExitCode -ne 0) { throw "Fixture install failed: $($taskProcess.ExitCode)" }
        $taskRegistration = Get-ItemProperty -LiteralPath $taskUninstallKey
        if ($taskRegistration.DisplayVersion -ne $taskVersion -or $taskRegistration.InstallLocation.Trim('"') -ne $taskInstall) {
            throw 'Installer registered the wrong version or directory.'
        }
        foreach ($taskSource in Get-ChildItem -LiteralPath (Join-Path $taskRepository 'src-tauri/target/installer/integration') -File -Recurse) {
            $taskRelative = [IO.Path]::GetRelativePath((Join-Path $taskRepository 'src-tauri/target/installer'), $taskSource.FullName)
            $taskInstalled = Join-Path $taskInstall $taskRelative
            if ((Get-FileHash -LiteralPath $taskSource.FullName).Hash -ne (Get-FileHash -LiteralPath $taskInstalled).Hash) {
                throw "Installed resource differs: $taskRelative"
            }
        }
        if ($taskVersion -eq '0.6.0-dev.0') {
            foreach ($taskEntry in $taskSentinels.GetEnumerator()) {
                $taskFile = Join-Path $taskData $taskEntry.Key
                New-Item -ItemType Directory -Force -Path (Split-Path $taskFile) | Out-Null
                [IO.File]::WriteAllText($taskFile, $taskEntry.Value)
            }
            [IO.File]::WriteAllText((Join-Path $taskInstall 'unowned.txt'), 'unowned fixture')
        }
        Assert-Retained
        Write-Output "Verified install and retained fixtures: $taskVersion"
    }

    $taskUninstaller = Join-Path $taskInstall 'uninstall.exe'
    $taskAppFixture = Join-Path $taskEvidence 'starframe.exe'
    Copy-Item -LiteralPath (Join-Path $env:SystemRoot 'System32/ping.exe') -Destination $taskAppFixture
    $taskRunning = Start-Process -FilePath $taskAppFixture -ArgumentList @('-t', '127.0.0.1') -WindowStyle Hidden -PassThru
    try {
        foreach ($taskSetup in @($taskInstaller, $taskUninstaller)) {
            # Run the refusal check in place so NSIS returns the uninstaller's exit code, not its temporary-copy launcher code.
            $taskBlockedArguments = if ($taskSetup -eq $taskUninstaller) { @('/S', "_?=$taskInstall") } else { @('/S') }
            $taskBlocked = Start-Process -FilePath $taskSetup -ArgumentList $taskBlockedArguments -WindowStyle Hidden -PassThru -Wait
            if ($taskBlocked.ExitCode -eq 0 -or $taskRunning.HasExited) {
                throw 'Setup must refuse a running app without terminating it.'
            }
            if (!(Test-Path -LiteralPath (Join-Path $taskInstall 'starframe.exe'))) {
                throw 'Setup removed the installed executable while the app was running.'
            }
            Assert-Retained
        }
    }
    finally { if (!$taskRunning.HasExited) { $taskRunning.Kill(); $taskRunning.WaitForExit() } }

    $taskProcess = Start-Process -FilePath $taskUninstaller -ArgumentList '/S' -WindowStyle Hidden -PassThru -Wait
    if ($taskProcess.ExitCode -ne 0) { throw "Fixture uninstall failed: $($taskProcess.ExitCode)" }
    if ((Test-Path -LiteralPath $taskUninstallKey) -or (Test-Path -LiteralPath (Join-Path $taskInstall 'starframe.exe'))) {
        throw 'App removal or registration cleanup did not finish.'
    }
    Assert-Retained
    $taskRemaining = @(Get-ChildItem -LiteralPath $taskInstall -File -Recurse)
    if ($taskRemaining.Count -ne 1 -or $taskRemaining[0].Name -ne 'unowned.txt') {
        throw 'Owned installation files remain after uninstall.'
    }
    @{
        ordinaryUser = $true
        versions = @('0.6.0-dev.0', '0.6.0-dev.1')
        runtimeHashesMatched = $true
        appDataRetained = $true
        unownedFileRetained = $true
        appAndRegistrationRemoved = $true
        runningAppRetained = $true
        gameLaunched = $false
        limitation = 'NSIS metadata upgrade over the same executable; no application/database migration or absent-WebView2 test.'
    } | ConvertTo-Json | Set-Content -Encoding utf8 (Join-Path $taskEvidence 'result.json')

    # Delete only the synthetic files above; nonrecursive removal refuses unexpected contents.
    foreach ($taskEntry in $taskSentinels.GetEnumerator()) {
        Remove-Item -LiteralPath (Join-Path $taskData $taskEntry.Key)
    }
    foreach ($taskSubdir in @('sqlite', 'library')) { [IO.Directory]::Delete((Join-Path $taskData $taskSubdir), $false) }
    [IO.Directory]::Delete($taskData, $false)
    Remove-Item -LiteralPath (Join-Path $taskProductKey $taskProduct)
    Remove-Item -LiteralPath $taskProductKey
    Write-Output "Installer fixture passed; evidence retained beneath test-results/0.6.0-packaging."
}
finally { Pop-Location }
