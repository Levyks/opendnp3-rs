# Build multi-OS Maven JARs (windows-x86_64 + linux-x86_64) for com.levyks:opendnp3 and opendnp3-tcp.
# Windows JNI DLL is built on this machine via `boltffi pack java --release`.
# Linux JNI .so and Maven packaging/deployment run in Docker (Debian bookworm, glibc 2.36, JDK 17 + Maven).
#
# Build + verify locally (no upload):
#   ./scripts/pack-maven.ps1 -SkipUpload
#
# Publish to Maven Central (requires Sonatype Central Portal user token + ASCII-armored GPG private key):
#   $env:CENTRAL_USERNAME = '...'
#   $env:CENTRAL_PASSWORD = '...'
#   $env:GPG_PRIVATE_KEY  = Get-Content -Raw .\my-key.asc
#   $env:GPG_PASSPHRASE   = '...'
#   ./scripts/pack-maven.ps1

param(
    [string]$JavaHome = 'C:\Users\Levyks\.gradle\jdks\azul_systems__inc_-21-amd64-windows.2',
    [switch]$SkipWindowsBuild,
    [switch]$SkipLinuxBuild,
    [switch]$SkipUpload
)

$ErrorActionPreference = 'Stop'
$Root = Split-Path -Parent $PSScriptRoot
$Ffi = Join-Path $Root 'opendnp3-ffi'

if (-not $SkipWindowsBuild) {
    $vcvars = 'C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools\VC\Auxiliary\Build\vcvars64.bat'
    if (Test-Path $vcvars) {
        cmd /c "set `"JAVA_HOME=$JavaHome`" && set `"PATH=$JavaHome\bin;%PATH%`" && call `"$vcvars`" >nul && cd /d `"$Ffi`" && boltffi pack java --release"
        if ($LASTEXITCODE -ne 0) { throw "Windows boltffi pack java failed: $LASTEXITCODE" }
    } else {
        throw "vcvars64.bat not found at $vcvars"
    }
}

docker build -t opendnp3-maven-linux -f (Join-Path $PSScriptRoot 'linux-maven.Dockerfile') $PSScriptRoot
if ($LASTEXITCODE -ne 0) { throw "Linux Maven image build failed: $LASTEXITCODE" }

if (-not $SkipLinuxBuild) {
    $linuxCmd = @'
set -euo pipefail
cargo build -p opendnp3-ffi --release
mkdir -p /src/opendnp3-ffi/dist/java/native/linux-x86_64
gcc -O2 -shared -fPIC \
  -I"$JAVA_HOME/include" \
  -I"$JAVA_HOME/include/linux" \
  -I/src/opendnp3-ffi/dist/java/jni \
  /src/opendnp3-ffi/dist/java/jni/jni_glue.c \
  -Wl,--whole-archive /tmp/opendnp3-target/release/libopendnp3_ffi.a -Wl,--no-whole-archive \
  -lpthread -ldl -lm -Wl,-s \
  -o /src/opendnp3-ffi/dist/java/native/linux-x86_64/libopendnp3_ffi_jni.so
ls -lh /src/opendnp3-ffi/dist/java/native/linux-x86_64/libopendnp3_ffi_jni.so
'@
    $linuxCmd = $linuxCmd -replace "`r`n", "`n"
    docker run --rm `
        -v "${Root}:/src" `
        -e CARGO_TARGET_DIR=/tmp/opendnp3-target `
        -w /src `
        opendnp3-maven-linux `
        bash -c $linuxCmd
    if ($LASTEXITCODE -ne 0) { throw "Linux JNI .so build failed: $LASTEXITCODE" }
}

if ($SkipUpload) {
    $pkgCmd = @'
set -euo pipefail
mvn -B -f /src/bindings/java/pom.xml -pl .,opendnp3,opendnp3-tcp,playground clean package
java -cp /src/bindings/java/playground/target/opendnp3-playground-0.1.1.jar:/src/bindings/java/opendnp3-tcp/target/opendnp3-tcp-0.1.1.jar:/src/bindings/java/opendnp3/target/opendnp3-0.1.1.jar Playground
'@
    $pkgCmd = $pkgCmd -replace "`r`n", "`n"
    docker run --rm -v "${Root}:/src" -w /src opendnp3-maven-linux bash -c $pkgCmd
    if ($LASTEXITCODE -ne 0) { throw "Maven package + Linux playground verification failed: $LASTEXITCODE" }
    return
}

if (-not $env:CENTRAL_USERNAME -or -not $env:CENTRAL_PASSWORD) {
    throw 'Set CENTRAL_USERNAME and CENTRAL_PASSWORD (Sonatype Central Portal user token) before deploying.'
}
if (-not $env:GPG_PRIVATE_KEY) {
    throw 'Set GPG_PRIVATE_KEY (ASCII-armored private key) and GPG_PASSPHRASE before deploying.'
}

$deployCmd = @'
set -euo pipefail
mkdir -p /root/.m2
cat > /root/.m2/settings.xml <<EOF
<settings>
  <servers>
    <server>
      <id>central</id>
      <username>${CENTRAL_USERNAME}</username>
      <password>${CENTRAL_PASSWORD}</password>
    </server>
  </servers>
</settings>
EOF
printf '%s' "$GPG_PRIVATE_KEY" | gpg --batch --import
mvn -B -f /src/bindings/java/pom.xml -P central-release -pl .,opendnp3,opendnp3-tcp \
  -Dgpg.passphrase="${GPG_PASSPHRASE:-}" \
  clean deploy
'@
$deployCmd = $deployCmd -replace "`r`n", "`n"
docker run --rm `
    -v "${Root}:/src" `
    -e CENTRAL_USERNAME `
    -e CENTRAL_PASSWORD `
    -e GPG_PRIVATE_KEY `
    -e GPG_PASSPHRASE `
    -w /src `
    opendnp3-maven-linux `
    bash -c $deployCmd
if ($LASTEXITCODE -ne 0) { throw "Maven Central deploy failed: $LASTEXITCODE" }
