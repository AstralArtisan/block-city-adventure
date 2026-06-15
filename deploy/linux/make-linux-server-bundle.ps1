param(
    [string]$OutputDir = "",
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"

$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$OutputDir = if ([string]::IsNullOrWhiteSpace($OutputDir)) {
    Join-Path $projectRoot "deploy-bundle"
} else {
    $OutputDir
}

$image = "rust:1.88-bookworm"
$installDeps = "apt-get update && apt-get install -y --no-install-recommends pkg-config libasound2-dev libudev-dev libx11-dev libxi-dev libxrandr-dev libxcursor-dev libxinerama-dev libgl1-mesa-dev ca-certificates"
$runtimeLibDir = "/work/target/linux-runtime-libs"
$copyRuntimeLibs = "mkdir -p $runtimeLibDir && cp -L /lib/x86_64-linux-gnu/libasound.so.2 /lib/x86_64-linux-gnu/libudev.so.1 /lib/x86_64-linux-gnu/libgcc_s.so.1 $runtimeLibDir/"
$buildCmd = "$installDeps && cargo build --release --bin server && $copyRuntimeLibs"

if (-not $SkipBuild) {
    $dockerArgs = @(
        "run",
        "--rm",
        "-v",
        "${projectRoot}:/work",
        "-w",
        "/work",
        $image,
        "bash",
        "-c",
        $buildCmd
    )
    & docker @dockerArgs
    if ($LASTEXITCODE -ne 0) {
        throw "docker build failed with exit code $LASTEXITCODE"
    }
}

$serverBin = Join-Path $projectRoot "target\release\server"
if (-not (Test-Path -LiteralPath $serverBin)) {
    throw "Linux server binary was not produced: $serverBin"
}
$hostRuntimeLibDir = Join-Path $projectRoot "target\linux-runtime-libs"
if (-not (Test-Path -LiteralPath $hostRuntimeLibDir)) {
    throw "Runtime library directory was not produced: $hostRuntimeLibDir"
}

$stagingRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("block-city-server-linux-" + [guid]::NewGuid().ToString("N"))
$appRoot = Join-Path $stagingRoot "block-city-adventure"
$assetsRoot = Join-Path $appRoot "assets"
$deployRoot = Join-Path $appRoot "deploy\linux"
$libRoot = Join-Path $appRoot "lib"

New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
New-Item -ItemType Directory -Path $appRoot -Force | Out-Null
New-Item -ItemType Directory -Path $assetsRoot -Force | Out-Null
New-Item -ItemType Directory -Path $deployRoot -Force | Out-Null
New-Item -ItemType Directory -Path $libRoot -Force | Out-Null

Copy-Item -LiteralPath $serverBin -Destination (Join-Path $appRoot "server") -Force
Copy-Item -Path (Join-Path $hostRuntimeLibDir "*") -Destination $libRoot -Force
Copy-Item -LiteralPath (Join-Path $projectRoot "assets\configs") -Destination (Join-Path $assetsRoot "configs") -Recurse -Force
Copy-Item -LiteralPath (Join-Path $projectRoot "deploy\linux\run-server.sh") -Destination (Join-Path $appRoot "run-server.sh") -Force
Copy-Item -LiteralPath (Join-Path $projectRoot "deploy\linux\run-server.sh") -Destination (Join-Path $deployRoot "run-server.sh") -Force
Copy-Item -LiteralPath (Join-Path $projectRoot "deploy\linux\server.env.example") -Destination $deployRoot -Force
Copy-Item -LiteralPath (Join-Path $projectRoot "deploy\linux\block-city-server.service") -Destination $deployRoot -Force

$readme = @'
Block City Adventure dedicated server

Run coop server (UDP 3457):
  chmod +x ./run-server.sh ./server
  ./run-server.sh --coop-server

Run pvp server (UDP 3456):
  chmod +x ./run-server.sh ./server
  ./run-server.sh --pvp-server

Optional custom port:
  ./run-server.sh --coop-server --port 3457
  ./run-server.sh --pvp-server --port 3456

Open the matching UDP port in your cloud security group and server firewall.
'@
Set-Content -LiteralPath (Join-Path $appRoot "README-server.txt") -Value $readme -Encoding ASCII

$tarPath = Join-Path $OutputDir "block-city-server-linux.tar.gz"
$zipPath = Join-Path $OutputDir "block-city-server-linux.zip"
if (Test-Path -LiteralPath $tarPath) { Remove-Item -LiteralPath $tarPath -Force }
if (Test-Path -LiteralPath $zipPath) { Remove-Item -LiteralPath $zipPath -Force }

tar -czf $tarPath -C $stagingRoot "block-city-adventure"
Compress-Archive -Path (Join-Path $appRoot "*") -DestinationPath $zipPath -CompressionLevel Optimal
Remove-Item -LiteralPath $stagingRoot -Recurse -Force

$tarInfo = Get-Item $tarPath
$zipInfo = Get-Item $zipPath
Write-Output "Created Linux server bundles:"
Write-Output $tarInfo.FullName
Write-Output ("tar.gz size: {0:N2} MB" -f ($tarInfo.Length / 1MB))
Write-Output $zipInfo.FullName
Write-Output ("zip size: {0:N2} MB" -f ($zipInfo.Length / 1MB))
