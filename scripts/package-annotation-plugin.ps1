param(
    [string]$OutputDirectory = "target",
    [string]$Architecture = "x64"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot

# Version embedded in the archive filename. Defaults to the plugin's own
# manifest version so the community index can reference a fixed URL per
# version; may be overridden for local testing.
function Get-IndexArchitecture([string]$arch) {
    switch ($arch.ToLowerInvariant()) {
        { $_ -in @("x64", "amd64", "x86_64") } { return "x86_64" }
        { $_ -in @("arm64", "aarch64") } { return "aarch64" }
        default { throw "Unsupported target architecture $arch" }
    }
}

Push-Location $root
try {
    cargo build --release -p lumia-plugin-annotation
    if ($LASTEXITCODE -ne 0) { throw "Annotation plugin build failed" }

    $packageName = "Lumia-Annotation-windows-$Architecture"
    $staging = Join-Path $OutputDirectory $packageName
    $plugin = Join-Path $staging "lumia-plugin-annotation"
    $archive = Join-Path $OutputDirectory "$packageName.lumiaplugin"
    $zipArchive = "$archive.zip"
    if (Test-Path $staging) {
        Remove-Item -Recurse -Force $staging
    }
    if (Test-Path $archive) {
        Remove-Item -Force $archive
    }
    if (Test-Path $zipArchive) {
        Remove-Item -Force $zipArchive
    }
    New-Item -ItemType Directory -Force -Path $plugin | Out-Null
    Copy-Item target/release/lumia-plugin-annotation.exe $plugin/
    Copy-Item plugins/lumia-plugin-annotation/lumia.plugin.json $plugin/
    Copy-Item plugins/lumia-plugin-annotation/lumia.plugin.sig $plugin/

    foreach ($required in @(
        "lumia-plugin-annotation.exe",
        "lumia.plugin.json",
        "lumia.plugin.sig"
    )) {
        if (-not (Test-Path (Join-Path $plugin $required))) {
            throw "Annotation plugin package is missing $required"
        }
    }

    $appVersion = (Select-String -Path crates/lumia-app/Cargo.toml -Pattern '^version = "([^"]+)"$').Matches[0].Groups[1].Value
    $pluginApiVersion = (Select-String -Path crates/lumia-plugin-api/src/rpc.rs -Pattern 'PROTOCOL_VERSION: u32 = ([0-9]+)').Matches[0].Groups[1].Value
    node scripts/sign-plugin-package.mjs `
        --root $staging `
        --install-directory lumia-plugin-annotation `
        --plugin-id lumia.annotation `
        --target-os windows `
        --target-arch $Architecture `
        --minimum-lumia-version $appVersion `
        --plugin-api-version $pluginApiVersion
    if ($LASTEXITCODE -ne 0) { throw "Annotation plugin package signing failed" }

    foreach ($metadata in @("lumia.package.json", "lumia.package.sig")) {
        if (-not (Test-Path (Join-Path $staging $metadata))) {
            throw "Annotation plugin package is missing $metadata"
        }
    }

    Compress-Archive -Path "$staging/*" -DestinationPath $zipArchive
    Move-Item -LiteralPath $zipArchive -Destination $archive
    Write-Host "Created $archive"

    $pluginVersion = (Get-Content plugins/lumia-plugin-annotation/lumia.plugin.json -Raw | ConvertFrom-Json).version
    $indexArch = Get-IndexArchitecture $Architecture
    $versioned = Join-Path $OutputDirectory "Lumia-Annotation-$pluginVersion-windows-$indexArch.lumiaplugin"
    Copy-Item -LiteralPath $archive -Destination $versioned
    Write-Host "Created $versioned"
} finally {
    Pop-Location
}
