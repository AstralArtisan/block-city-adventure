param(
    [string]$OutputDir = ""
)

$ErrorActionPreference = "Stop"

$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$OutputDir = if ([string]::IsNullOrWhiteSpace($OutputDir)) {
    Join-Path $projectRoot "deploy-bundle"
} else {
    $OutputDir
}
$stagingRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("block-city-adventure-source-" + [guid]::NewGuid().ToString("N"))
$zipPath = Join-Path $OutputDir "block-city-adventure-source.zip"

if (Test-Path $zipPath) {
    Remove-Item -LiteralPath $zipPath -Force
}

New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
New-Item -ItemType Directory -Path $stagingRoot -Force | Out-Null

function Copy-RelativePath {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RelativePath
    )

    $sourcePath = Join-Path $projectRoot $RelativePath
    if (-not (Test-Path $sourcePath)) {
        return
    }

    $destinationPath = Join-Path $stagingRoot $RelativePath
    $destinationParent = Split-Path -Parent $destinationPath
    if ($destinationParent) {
        New-Item -ItemType Directory -Path $destinationParent -Force | Out-Null
    }

    Copy-Item -LiteralPath $sourcePath -Destination $destinationPath -Recurse -Force
}

$pathsToCopy = @(
    "Cargo.toml",
    "Cargo.lock",
    ".cargo\config.toml",
    "src",
    "assets\configs",
    "deploy\linux\run-server.sh",
    "deploy\linux\block-city-server.service",
    "deploy\linux\server.env.example",
    "docs\aliyun_linux_deploy.md"
)

foreach ($path in $pathsToCopy) {
    Copy-RelativePath -RelativePath $path
}

Compress-Archive -Path (Join-Path $stagingRoot "*") -DestinationPath $zipPath -CompressionLevel Optimal
Remove-Item -LiteralPath $stagingRoot -Recurse -Force

$zipInfo = Get-Item $zipPath
Write-Output "Created source bundle:"
Write-Output $zipInfo.FullName
Write-Output ("Size: {0:N2} MB" -f ($zipInfo.Length / 1MB))
