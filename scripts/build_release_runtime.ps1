param(
    [Parameter(Mandatory = $true)][string] $GameManagedPath,
    [Parameter(Mandatory = $true)][string] $BootstrapPath,
    [Parameter(Mandatory = $true)][string] $Output
)
$ErrorActionPreference = 'Stop'
$taskRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$taskOutput = [IO.Path]::GetFullPath($Output)
if (Test-Path -LiteralPath $taskOutput) { throw 'Use a new output directory.' }
$taskChanged = git -C $taskRoot status --porcelain --untracked-files=all -- runtime scripts/build_release_runtime.ps1 scripts/release_runtime.py scripts/prepare_desktop_runtime.py LICENSE docs/notices/runtime-dependencies.txt
if ($taskChanged) { throw 'Commit the runtime source and build scripts before creating a release input.' }
New-Item -ItemType Directory -Path $taskOutput | Out-Null
Push-Location (Join-Path $taskRoot 'runtime')
try {
    $taskSdk = (Get-Content -Raw global.json | ConvertFrom-Json).sdk.version
    if ((dotnet --version) -ne $taskSdk) { throw 'Use the pinned runtime .NET SDK.' }
    dotnet build Starframe.Bootstrap/Starframe.Bootstrap.csproj --configuration Release -p:RestoreLockedMode=true --output "$taskOutput/build" "-p:GameManagedPath=$GameManagedPath" "-p:BootstrapPath=$BootstrapPath" "-p:PathMap=$taskRoot=/_/" -p:ContinuousIntegrationBuild=true
    if ($LASTEXITCODE -ne 0) { throw 'Local runtime build failed.' }
    python ../scripts/release_runtime.py pack --runtime "$taskOutput/build" --archive "$taskOutput/runtime.zip" --manifest "$taskOutput/runtime.json"
    if ($LASTEXITCODE -ne 0) { throw 'Runtime packaging failed.' }
} finally { Pop-Location }
