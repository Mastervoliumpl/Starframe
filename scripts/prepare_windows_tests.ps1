$ErrorActionPreference = 'Stop'
function Get-WebViewVersion {
    foreach ($taskKey in @(
        'HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}',
        'HKCU:\Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}'
    )) {
        $taskVersion = (Get-ItemProperty -LiteralPath $taskKey -Name pv -ErrorAction SilentlyContinue).pv
        if ($taskVersion -and $taskVersion -ne '0.0.0.0') { return $taskVersion }
    }
}
$taskVersion = Get-WebViewVersion
if (!$taskVersion) {
    if ($env:GITHUB_ACTIONS -ne 'true') { throw 'WebView2 is missing. Install the Microsoft Evergreen WebView2 Runtime before running native tests.' }
    $taskInstaller = Join-Path $env:RUNNER_TEMP 'MicrosoftEdgeWebview2Setup.exe'
    Invoke-WebRequest -Uri 'https://go.microsoft.com/fwlink/p/?LinkId=2124703' -OutFile $taskInstaller
    $taskSignature = Get-AuthenticodeSignature -LiteralPath $taskInstaller
    if ($taskSignature.Status -ne 'Valid' -or $taskSignature.SignerCertificate.Subject -notmatch 'O=Microsoft Corporation') { throw 'The WebView2 bootstrapper does not have a valid Microsoft signature.' }
    $taskInstall = Start-Process -FilePath $taskInstaller -ArgumentList '/silent', '/install' -WindowStyle Hidden -PassThru -Wait
    if ($taskInstall.ExitCode -ne 0) { throw "WebView2 installation failed: $($taskInstall.ExitCode)" }
    $taskVersion = Get-WebViewVersion
    if (!$taskVersion) { throw 'WebView2 is still unavailable after installation.' }
}
Write-Output "WebView2 Runtime: $taskVersion"
Write-Output "Windows session: $([System.Diagnostics.Process]::GetCurrentProcess().SessionId); interactive: $([Environment]::UserInteractive)"
$taskPrincipal = [Security.Principal.WindowsPrincipal]::new([Security.Principal.WindowsIdentity]::GetCurrent())
Write-Output "Elevated: $($taskPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator))"
