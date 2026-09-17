param()

$ErrorActionPreference = 'Stop'
$taskRepository = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$taskCompiler = Join-Path $env:LOCALAPPDATA 'tauri/NSIS/makensis.exe'
if (!(Test-Path -LiteralPath $taskCompiler)) { throw 'Bundle the NSIS installer before running maintenance tests.' }
$taskEvidence = Join-Path $taskRepository ('test-results/0.6.0-packaging/maintenance-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $taskEvidence | Out-Null
$taskTemplate = [IO.File]::ReadAllText((Join-Path $taskRepository 'src-tauri/windows/installer.nsi'))
$taskCallback = [regex]::Match($taskTemplate, '(?ms)^Function PageLeaveReinstall\r?\n.*?^FunctionEnd').Value
if (!$taskCallback) { throw 'Installer maintenance callback was not found.' }

# Execute the production callback in NSIS. Only the radio input and child uninstaller are fixtures.
$taskHarness = @'
Unicode true
Name "Starframe maintenance fixture"
OutFile "maintenance.exe"
RequestExecutionLevel user
SilentInstall silent
!include LogicLib.nsh
!include FileFunc.nsh
!define MAINBINARYNAME "fixture-app"
!define MANUPRODUCTKEY "Software\Starframe Maintenance Fixture"
!define UNINSTKEY "Software\Starframe Maintenance Fixture"
LoadLanguageFile "${NSISDIR}\Contrib\Language files\English.nlf"
LangString unableToUninstall 1033 "Fixture uninstall failed"
Var WixMode
Var UpdateMode
Var PassiveMode
Var FixtureChoice
!macro FixtureGetState handle result
  StrCpy ${result} $FixtureChoice
!macroend
!define NSD_GetState "!insertmacro FixtureGetState"
Function .onInit
  SetShellVarContext current
  StrCpy $INSTDIR $EXEDIR
  ${GetParameters} $9
  ${GetOptions} $9 "/VERSION=" $R0
  ${GetOptions} $9 "/CHOICE=" $FixtureChoice
  ${GetOptions} $9 "/UPDATE=" $UpdateMode
  ${GetOptions} $9 "/RESULT=" $8
  StrCpy $WixMode 0
  StrCpy $PassiveMode 0
  WriteRegStr HKCU "${MANUPRODUCTKEY}" "" "$EXEDIR"
  WriteRegStr HKCU "${UNINSTKEY}" "UninstallString" '$\"$EXEDIR\child.exe$\" /RESULT=$8'
  Call PageLeaveReinstall
  FileOpen $7 "$EXEDIR\continued.txt" w
  FileWrite $7 "installation continued"
  FileClose $7
  Quit
FunctionEnd
Section
SectionEnd
'@
$taskChild = @'
Unicode true
Name "Starframe maintenance child fixture"
OutFile "child.exe"
RequestExecutionLevel user
SilentInstall silent
!include FileFunc.nsh
Function .onInit
  ${GetParameters} $0
  FileOpen $1 "$EXEDIR\child-arguments.txt" w
  FileWrite $1 $0
  FileClose $1
  ${GetOptions} $0 "/RESULT=" $2
  StrCpy $2 $2 6 ; Ignore the parent's trailing _?= directory argument.
  StrCmp $2 "cancel" cancelled
  Delete "$EXEDIR\fixture-app.exe"
  SetErrorLevel 0
  Quit
  cancelled:
  SetErrorLevel 1
  Quit
FunctionEnd
Section
SectionEnd
'@
$taskRegistry = 'HKCU:\Software\Starframe Maintenance Fixture'
if (Test-Path -LiteralPath $taskRegistry) { throw 'A previous maintenance fixture remains; inspect it before retrying.' }
try {
    [IO.File]::WriteAllText((Join-Path $taskEvidence 'maintenance.nsi'), $taskHarness + "`n" + $taskCallback)
    [IO.File]::WriteAllText((Join-Path $taskEvidence 'child.nsi'), $taskChild)
    foreach ($taskScript in 'maintenance', 'child') {
        & $taskCompiler '/V2' (Join-Path $taskEvidence "$taskScript.nsi") *> (Join-Path $taskEvidence "$taskScript-build.log")
        if ($LASTEXITCODE -ne 0) { throw "NSIS $taskScript fixture failed to compile; inspect its retained log." }
    }
    $taskCases = @(
        @{ name = 'uninstall'; version = 0; choice = 0; update = 0; result = 'success'; continues = $false; child = $true; retains = $false; preserves = $false },
        @{ name = 'reinstall'; version = 0; choice = 1; update = 0; result = 'success'; continues = $true; child = $false; retains = $true; preserves = $false },
        @{ name = 'upgrade'; version = 1; choice = 1; update = 0; result = 'success'; continues = $true; child = $true; retains = $false; preserves = $true },
        @{ name = 'replacement'; version = -1; choice = 1; update = 0; result = 'success'; continues = $true; child = $true; retains = $false; preserves = $true },
        @{ name = 'updater'; version = 1; choice = 0; update = 1; result = 'success'; continues = $true; child = $false; retains = $true; preserves = $false },
        @{ name = 'cancel'; version = 0; choice = 0; update = 0; result = 'cancel'; continues = $false; child = $true; retains = $true; preserves = $false }
    )
    foreach ($taskCase in $taskCases) {
        $taskDirectory = Join-Path $taskEvidence $taskCase.name
        New-Item -ItemType Directory -Path $taskDirectory | Out-Null
        foreach ($taskName in 'maintenance.exe', 'child.exe') { Copy-Item -LiteralPath (Join-Path $taskEvidence $taskName) -Destination (Join-Path $taskDirectory $taskName) }
        [IO.File]::WriteAllText((Join-Path $taskDirectory 'fixture-app.exe'), 'inert fixture')
        $taskArguments = @("/VERSION=$($taskCase.version)", "/CHOICE=$($taskCase.choice)", "/UPDATE=$($taskCase.update)", "/RESULT=$($taskCase.result)")
        $taskProcess = Start-Process -FilePath (Join-Path $taskDirectory 'maintenance.exe') -ArgumentList $taskArguments -WindowStyle Hidden -PassThru
        if (!$taskProcess.WaitForExit(30000)) { $taskProcess.Kill(); throw 'Maintenance fixture did not finish within 30 seconds.' }
        if ((Test-Path (Join-Path $taskDirectory 'continued.txt')) -ne $taskCase.continues) { throw "$($taskCase.name): unexpected installation continuation" }
        if ((Test-Path (Join-Path $taskDirectory 'fixture-app.exe')) -ne $taskCase.retains) { throw "$($taskCase.name): unexpected app retention" }
        $taskChildArguments = Join-Path $taskDirectory 'child-arguments.txt'
        if ((Test-Path $taskChildArguments) -ne $taskCase.child) { throw "$($taskCase.name): unexpected child execution" }
        if ($taskCase.child -and ([IO.File]::ReadAllText($taskChildArguments).Contains(' /UPDATE')) -ne $taskCase.preserves) { throw "$($taskCase.name): unexpected data-retention argument" }
        Remove-Item -LiteralPath $taskRegistry
        Write-Output "Maintenance callback passed: $($taskCase.name)"
    }
}
finally {
    if (Test-Path -LiteralPath $taskRegistry) { Remove-Item -LiteralPath $taskRegistry }
}
