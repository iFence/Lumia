param(
    [string]$OutputDirectory = "target",
    [string]$Architecture = "x64"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Push-Location $root
try {
    cargo build --release -p lumia-plugin-annotation
    if ($LASTEXITCODE -ne 0) { throw "Annotation plugin build failed" }

    $packageName = "Lumia-Annotation-windows-$Architecture"
    $staging = Join-Path $OutputDirectory $packageName
    $plugin = Join-Path $staging "lumia-plugin-annotation"
    $archive = Join-Path $OutputDirectory "$packageName.zip"
    if (Test-Path $staging) {
        Remove-Item -Recurse -Force $staging
    }
    if (Test-Path $archive) {
        Remove-Item -Force $archive
    }
    New-Item -ItemType Directory -Force -Path $plugin | Out-Null
    Copy-Item target/release/lumia-plugin-annotation.exe $plugin/
    Copy-Item plugins/lumia-plugin-annotation/lumia.plugin.json $plugin/
    Copy-Item plugins/lumia-plugin-annotation/lumia.plugin.sig $plugin/
    Copy-Item plugins/lumia-plugin-annotation/assets $plugin/ -Recurse

    foreach ($required in @(
        "lumia-plugin-annotation.exe",
        "lumia.plugin.json",
        "lumia.plugin.sig",
        "assets/pin.svg"
    )) {
        if (-not (Test-Path (Join-Path $plugin $required))) {
            throw "Annotation plugin package is missing $required"
        }
    }

    Compress-Archive -Path "$staging/*" -DestinationPath $archive
    Write-Host "Created $archive"
} finally {
    Pop-Location
}
