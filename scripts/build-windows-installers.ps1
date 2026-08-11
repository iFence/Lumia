param(
    [string]$OutputDirectory = "target/wix"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Push-Location $root
try {
    & "$PSScriptRoot/verify-windows-icon.ps1"

    $metadata = cargo metadata --no-deps --format-version 1 | ConvertFrom-Json
    $app = $metadata.packages | Where-Object name -eq "lumia-app" | Select-Object -First 1
    if (-not $app) {
        throw "Could not determine the lumia-app package version"
    }
    $version = $app.version
    New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null

    cargo build --release -p lumia-app -p lumia-svg-thumbnail -p lumia-plugin-photoshop -p lumia-plugin-jpeg-xl -p lumia-plugin-jpeg2000
    if ($LASTEXITCODE -ne 0) { throw "Release binary build failed" }

    $enUsMsi = Join-Path $OutputDirectory "Lumia-$version-x64-en-US.msi"
    $zhCnMsi = Join-Path $OutputDirectory "Lumia-$version-x64-zh-CN.msi"
    cargo wix -p lumia-app --no-build `
        --culture en-us `
        --locale crates/lumia-app/wix/en-US.wxl `
        -C '-dProductLanguage=1033' `
        -C '-dProductCodepage=1252' `
        --output $enUsMsi
    if ($LASTEXITCODE -ne 0) { throw "English MSI build failed" }

    cargo wix -p lumia-app --no-build `
        --culture zh-cn `
        --locale crates/lumia-app/wix/zh-CN.wxl `
        -C '-dProductLanguage=2052' `
        -C '-dProductCodepage=936' `
        --output $zhCnMsi
    if ($LASTEXITCODE -ne 0) { throw "Simplified Chinese MSI build failed" }

    $env:LUMIA_MSI_EN_US = (Resolve-Path $enUsMsi).Path
    $env:LUMIA_MSI_ZH_CN = (Resolve-Path $zhCnMsi).Path
    cargo build --release -p lumia-setup
    if ($LASTEXITCODE -ne 0) { throw "Setup bootstrapper build failed" }

    $setup = Join-Path $OutputDirectory "Lumia-Setup-$version-x64.exe"
    Copy-Item target/release/lumia-setup.exe $setup -Force
    Write-Host "Built Windows packages in $OutputDirectory"
    Write-Host "  $setup"
    Write-Host "  $enUsMsi"
    Write-Host "  $zhCnMsi"
} finally {
    Pop-Location
}
