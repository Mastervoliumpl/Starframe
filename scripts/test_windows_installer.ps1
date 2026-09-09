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
    'sqlite/legacy-backup.db' = 'retained legacy fixture'
    'artifacts/imported.dll' = 'imported content fixture'
    'backups/settings-fixture.json' = 'saved settings fixture'
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

function Invoke-GameFixture([string] $taskMode) {
    & python scripts/installer_game_fixture.py $taskMode $taskEvidence
    if ($LASTEXITCODE -ne 0) { throw "Game fixture failed: $taskMode" }
}

function Assert-FailedUninstall {
    $taskFailed = Start-Process -FilePath $taskUninstaller -ArgumentList @('/S', "_?=$taskInstall") -WindowStyle Hidden -PassThru -Wait
    if ($taskFailed.ExitCode -eq 0) { throw 'Uninstall unexpectedly succeeded.' }
    foreach ($taskRetained in @($taskUninstallKey, (Join-Path $taskInstall 'starframe.exe'), (Join-Path $taskData 'sqlite/state.db'))) {
        if (!(Test-Path -LiteralPath $taskRetained)) { throw 'Failed uninstall removed the app, registration or database.' }
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
        foreach ($taskSource in Get-ChildItem -LiteralPath (Join-Path $taskRepository 'docs/notices') -File -Recurse) {
            $taskRelative = [IO.Path]::GetRelativePath((Join-Path $taskRepository 'docs/notices'), $taskSource.FullName)
            if ((Get-FileHash -LiteralPath $taskSource.FullName).Hash -ne (Get-FileHash -LiteralPath (Join-Path $taskInstall "notices/$taskRelative")).Hash) {
                throw "Installed notice differs: $taskRelative"
            }
        }
        $taskNoticeIndex = Join-Path $taskInstall 'THIRD_PARTY_NOTICES.md'
        if ((Get-FileHash -LiteralPath $taskNoticeIndex).Hash -ne (Get-FileHash -LiteralPath (Join-Path $taskRepository 'docs/THIRD_PARTY_NOTICES.md')).Hash) {
            throw 'Installed notice index differs.'
        }
        foreach ($taskLink in [regex]::Matches([IO.File]::ReadAllText($taskNoticeIndex), '\]\((notices/[^)]+)\)')) {
            if (!(Test-Path -LiteralPath (Join-Path $taskInstall $taskLink.Groups[1].Value) -PathType Leaf)) {
                throw 'An installed notice link has no target.'
            }
        }
        if ($taskVersion -eq '0.6.0-dev.0') {
            New-Item -ItemType Directory -Path $taskData | Out-Null
            $taskInit = Start-Process -FilePath (Join-Path $taskInstall 'starframe.exe') -ArgumentList @('--installer-uninstall', $taskIdentifier, 'keep') -WindowStyle Hidden -PassThru -Wait
            if ($taskInit.ExitCode -ne 0 -or !(Test-Path -LiteralPath (Join-Path $taskData 'sqlite/state.db'))) { throw 'Fixture database initialization failed.' }
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
    Invoke-GameFixture 'seed'
    $taskAppHash = (Get-FileHash -LiteralPath (Join-Path $taskInstall 'starframe.exe')).Hash
    $taskMissing = Join-Path $taskInstall 'integration/runtime/BepInEx/plugins/Starframe/Starframe.Runtime.dll'
    $taskRuntimeHash = (Get-FileHash -LiteralPath $taskMissing).Hash
    Remove-Item -LiteralPath $taskMissing
    [IO.File]::WriteAllText((Join-Path $taskInstall 'starframe.exe'), 'damaged app fixture')
    $taskRepair = Start-Process -FilePath $taskInstaller -ArgumentList @('/S', '/NS', "/D=$taskInstall") -WindowStyle Hidden -PassThru -Wait
    if ($taskRepair.ExitCode -ne 0 -or (Get-FileHash -LiteralPath (Join-Path $taskInstall 'starframe.exe')).Hash -ne $taskAppHash -or (Get-FileHash -LiteralPath $taskMissing).Hash -ne $taskRuntimeHash) {
        throw 'Same-version reinstall did not restore damaged and missing packaged files.'
    }
    Assert-Retained
    Invoke-GameFixture 'retained'

    $taskGame = (Resolve-Path (Join-Path $taskEvidence 'game')).Path
    $taskDisconnected = [IO.Path]::GetFullPath((Join-Path $taskEvidence 'game-disconnected'))
    foreach ($taskMove in @($taskGame, $taskDisconnected)) {
        if ([IO.Path]::GetDirectoryName($taskMove) -ne $taskEvidence) { throw 'Game fixture move escaped its evidence directory.' }
    }
    Move-Item -LiteralPath $taskGame -Destination $taskDisconnected
    try {
        Assert-FailedUninstall
        Assert-Retained
    }
    finally { Move-Item -LiteralPath $taskDisconnected -Destination $taskGame }
    Invoke-GameFixture 'retained'

    $taskLockedLoader = [IO.File]::Open((Join-Path $taskGame 'engine/winhttp.dll'), 'Open', 'Read', 'Read')
    try {
        Assert-FailedUninstall
        Assert-Retained
        Invoke-GameFixture 'retained'
    }
    finally { $taskLockedLoader.Dispose() }
    Write-Output 'Verified damaged-install reinstall, unavailable game reconnect and locked-file rollback.'

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

    Invoke-GameFixture 'interrupt'
    $taskProcess = Start-Process -FilePath $taskUninstaller -ArgumentList @('/S', '/KEEPDATA') -WindowStyle Hidden -PassThru -Wait
    if ($taskProcess.ExitCode -ne 0) { throw "Fixture uninstall failed: $($taskProcess.ExitCode)" }
    if ((Test-Path -LiteralPath $taskUninstallKey) -or (Test-Path -LiteralPath (Join-Path $taskInstall 'starframe.exe'))) {
        throw 'App removal or registration cleanup did not finish.'
    }
    Assert-Retained
    Invoke-GameFixture 'removed'
    $taskRemaining = @(Get-ChildItem -LiteralPath $taskInstall -File -Recurse)
    if ($taskRemaining.Count -ne 1 -or $taskRemaining[0].Name -ne 'unowned.txt') {
        throw 'Owned installation files remain after uninstall.'
    }
    $taskReinstall = Start-Process -FilePath $taskInstaller -ArgumentList @('/S', '/NS', "/D=$taskInstall") -WindowStyle Hidden -PassThru -Wait
    if ($taskReinstall.ExitCode -ne 0) { throw 'Reinstall over retained data failed.' }
    Assert-Retained
    $taskLockedData = [IO.File]::Open((Join-Path $taskData 'backups/settings-fixture.json'), 'Open', 'Read', 'Read')
    try {
        Assert-FailedUninstall
        if (Test-Path -LiteralPath (Join-Path $taskData 'artifacts')) { throw 'Fixture did not reach partial data deletion.' }
        if ([IO.File]::ReadAllText((Join-Path $taskData 'backups/settings-fixture.json')) -ne $taskSentinels['backups/settings-fixture.json']) { throw 'Locked data changed.' }
    }
    finally { $taskLockedData.Dispose() }
    $taskDelete = Start-Process -FilePath $taskUninstaller -ArgumentList '/S' -WindowStyle Hidden -PassThru -Wait
    if ($taskDelete.ExitCode -ne 0 -or (Test-Path -LiteralPath $taskUninstallKey) -or (Test-Path -LiteralPath (Join-Path $taskInstall 'starframe.exe'))) { throw 'Default uninstall failed.' }
    if (Test-Path -LiteralPath $taskData) { throw 'Default uninstall retained managed fixture data.' }
    if ([IO.File]::ReadAllText((Join-Path $taskInstall 'unowned.txt')) -ne 'unowned fixture') { throw 'Default uninstall changed an unowned file.' }
    Invoke-GameFixture 'removed'
    @{
        ordinaryUser = $true
        versions = @('0.6.0-dev.0', '0.6.0-dev.1')
        runtimeHashesMatched = $true
        noticeHashesMatched = $true
        explicitAppDataRetention = $true
        defaultAppDataDeletion = $true
        reinstallRetainedData = $true
        sameVersionRestoredDamagedFiles = $true
        unavailableGameReconnect = $true
        lockedGameFileRollback = $true
        interruptedJournalRecovery = $true
        partialDataRemovalRetry = $true
        unownedFileRetained = $true
        appAndRegistrationRemoved = $true
        runningAppRetained = $true
        gameLaunched = $false
        limitation = 'NSIS metadata upgrade over the same executable; interrupted journal state is seeded, not a process-kill test. No application/database migration, interactive UI or absent-WebView2 test.'
    } | ConvertTo-Json | Set-Content -Encoding utf8 (Join-Path $taskEvidence 'result.json')

    Remove-Item -LiteralPath (Join-Path $taskProductKey $taskProduct)
    Remove-Item -LiteralPath $taskProductKey
    Write-Output "Installer fixture passed; evidence retained beneath test-results/0.6.0-packaging."
}
finally { Pop-Location }
