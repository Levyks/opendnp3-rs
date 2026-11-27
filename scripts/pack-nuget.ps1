# Build multi-OS NuGet packages (win-x64 + linux-x64) for Levyks.OpenDnp3 and Levyks.OpenDnp3.Tcp.
# Windows native DLL is built on this machine. Linux .so is built in Docker (Debian bookworm, glibc 2.36).
# The API key stays in the environment. This script never stores it.
#
#   ./scripts/pack-nuget.ps1 -SkipUpload
#   $env:NUGET_API_KEY = 'oy2...'
#   ./scripts/pack-nuget.ps1

param(
    [string]$Source = 'https://api.nuget.org/v3/index.json',
    [switch]$SkipBuild,
    [switch]$SkipLinux,
    [switch]$SkipUpload
)

$ErrorActionPreference = 'Stop'
$Root = Split-Path -Parent $PSScriptRoot
$Ffi = Join-Path $Root 'opendnp3-ffi'
$CSharpDist = Join-Path $Ffi 'dist\csharp'
$PackagesDir = Join-Path $CSharpDist 'packages'
$Csproj = Join-Path $CSharpDist 'BoltFFI.CSharp.csproj'
$TcpCsproj = Join-Path $Root 'bindings\csharp\OpenDnp3.Tcp\OpenDnp3.Tcp.csproj'

if (-not $SkipBuild) {
    $vcvars = 'C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools\VC\Auxiliary\Build\vcvars64.bat'
    if (Test-Path $vcvars) {
        cmd /c "call `"$vcvars`" && set" | ForEach-Object {
            if ($_ -match '^(.*?)=(.*)$' -and $Matches[1] -notmatch '[() ]') {
                Set-Item -Path "Env:$($Matches[1])" -Value $Matches[2]
            }
        }
    }

    Push-Location $Ffi
    try {
        & boltffi pack csharp --release
        if ($LASTEXITCODE -ne 0) { throw "boltffi pack csharp failed: $LASTEXITCODE" }
    }
    finally { Pop-Location }

    $rids = @('win-x64')

    if (-not $SkipLinux) {
        docker build -t opendnp3-nuget-linux -f (Join-Path $PSScriptRoot 'linux-nuget.Dockerfile') $PSScriptRoot
        if ($LASTEXITCODE -ne 0) { throw "Linux NuGet image build failed: $LASTEXITCODE" }

        $linuxOut = Join-Path $CSharpDist 'runtimes\linux-x64\native'
        New-Item -ItemType Directory -Force -Path $linuxOut | Out-Null

        docker run --rm `
            -v "${Root}:/src" `
            -e CARGO_TARGET_DIR=/tmp/opendnp3-target `
            -w /src `
            opendnp3-nuget-linux `
            bash -c "cargo build -p opendnp3-ffi --release && cp /tmp/opendnp3-target/release/libopendnp3_ffi.so /src/opendnp3-ffi/dist/csharp/runtimes/linux-x64/native/"
        if ($LASTEXITCODE -ne 0) { throw "Linux libopendnp3_ffi.so build failed: $LASTEXITCODE" }

        $rids += 'linux-x64'
    }

    # Update RuntimeIdentifiers in generated csproj and re-pack with all native runtimes included
    $ridList = $rids -join ';'
    $csprojContent = Get-Content $Csproj -Raw
    $csprojContent = $csprojContent -replace '<RuntimeIdentifiers>[^<]*</RuntimeIdentifiers>', "<RuntimeIdentifiers>$ridList</RuntimeIdentifiers>"
    Set-Content -Path $Csproj -Value $csprojContent -NoNewline

    New-Item -ItemType Directory -Force -Path $PackagesDir | Out-Null
    & dotnet pack $Csproj -c Release -o $PackagesDir --source https://api.nuget.org/v3/index.json
    if ($LASTEXITCODE -ne 0) { throw "dotnet pack Levyks.OpenDnp3 failed: $LASTEXITCODE" }

    # Evict stale local NuGet cache for Levyks.OpenDnp3 so OpenDnp3.Tcp restores the freshly packed .nupkg
    $nugetGlobal = Join-Path $HOME '.nuget\packages\levyks.opendnp3'
    if (Test-Path $nugetGlobal) { Remove-Item -Recurse -Force $nugetGlobal }

    & dotnet pack $TcpCsproj -c Release -o $PackagesDir
    if ($LASTEXITCODE -ne 0) { throw "dotnet pack Levyks.OpenDnp3.Tcp failed: $LASTEXITCODE" }

    Write-Host "Packed NuGet packages in ${PackagesDir}:"
    Get-ChildItem $PackagesDir -Filter '*.nupkg' | ForEach-Object { Write-Host "  $($_.Name)" }
}

if ($SkipUpload) { return }

if (-not $env:NUGET_API_KEY) { throw 'Set NUGET_API_KEY before uploading to NuGet.' }
$nupkgs = @(Get-ChildItem $PackagesDir -Filter '*.nupkg' -ErrorAction SilentlyContinue)
if (-not $nupkgs) { throw "No .nupkg files found in $PackagesDir" }

foreach ($pkg in $nupkgs) {
    & dotnet nuget push $pkg.FullName --api-key $env:NUGET_API_KEY --source $Source --skip-duplicate
    if ($LASTEXITCODE -ne 0) { throw "dotnet nuget push failed for $($pkg.Name): $LASTEXITCODE" }
}
