param([switch] $ThemeOnly)

$ErrorActionPreference = 'Stop'
$taskRepository = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Push-Location $taskRepository
try {
    if (!$ThemeOnly) {
        & python scripts/check_installer.py
        if ($LASTEXITCODE -ne 0) { throw 'Installer preflight failed.' }
    }
    $taskLocator = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
    $taskVisualStudio = & $taskLocator -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if (!$taskVisualStudio) { throw 'Visual Studio C++ x86 tools are required for installer controls.' }
    $taskEnvironment = Join-Path $taskVisualStudio 'Common7/Tools/VsDevCmd.bat'
    $taskOutput = Join-Path $taskRepository 'src-tauri/target/installer/theme'
    New-Item -ItemType Directory -Path $taskOutput -Force | Out-Null
    $taskSource = Join-Path $taskRepository 'src-tauri/windows/theme/buttons.cpp'
    $taskExports = Join-Path $taskRepository 'src-tauri/windows/theme/buttons.def'
    Push-Location $taskOutput
    try {
        # The NSIS host is x86; the helper uses only Windows libraries, without a C runtime.
        $taskBuild = 'call "{0}" -no_logo -arch=x86 -host_arch=x64 >nul && cl /nologo /LD /O1 /GS- /Zl /W4 /WX /std:c++17 /DUNICODE /D_UNICODE "{1}" /link /NODEFAULTLIB /ENTRY:DllMain /DEF:"{2}" /OUT:StarframeTheme.dll user32.lib gdi32.lib comctl32.lib kernel32.lib' -f $taskEnvironment, $taskSource, $taskExports
        & $env:ComSpec /d /c $taskBuild
        if ($LASTEXITCODE -ne 0) { throw 'Installer button helper compilation failed.' }
    } finally { Pop-Location }
} finally { Pop-Location }
