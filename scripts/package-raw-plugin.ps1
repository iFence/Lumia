param(
    [Parameter(Mandatory = $true)][string]$BridgeLibrary,
    [Parameter(Mandatory = $true)][string]$LibRawLibrary,
    [Parameter(Mandatory = $true)][string]$LibRawLicenseDirectory,
    [string]$NativeDependencyDirectory = "",
    [string]$OutputDirectory = "target",
    [string]$Architecture = "x64"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$originalRustFlags = $env:RUSTFLAGS

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
    $staticCrtFlag = "-C target-feature=+crt-static"
    if (-not $originalRustFlags -or $originalRustFlags -notmatch "target-feature=\+crt-static") {
        $env:RUSTFLAGS = "$originalRustFlags $staticCrtFlag".Trim()
    }
    cargo build --release -p lumia-plugin-raw
    if ($LASTEXITCODE -ne 0) { throw "RAW plugin build failed" }

    foreach ($requiredPath in @($BridgeLibrary, $LibRawLibrary)) {
        if (-not (Test-Path -LiteralPath $requiredPath -PathType Leaf)) {
            throw "RAW native runtime is missing: $requiredPath"
        }
    }
    if ($NativeDependencyDirectory) {
        $resolvedNativeDependencyDirectory = (Resolve-Path -LiteralPath $NativeDependencyDirectory).Path
    } else {
        $resolvedNativeDependencyDirectory = Split-Path -Parent (Resolve-Path -LiteralPath $LibRawLibrary).Path
    }
    $nativeDependencies = @(
        (Join-Path $resolvedNativeDependencyDirectory "zlib1__.dll"),
        (Join-Path $resolvedNativeDependencyDirectory "libjpeg-9__.dll")
    )
    $nativeDependencyLicenses = @(
        (Join-Path $resolvedNativeDependencyDirectory "licenses/LICENSE.zlib"),
        (Join-Path $resolvedNativeDependencyDirectory "licenses/LICENSE.libjpeg")
    )
    foreach ($dependencyPath in @($nativeDependencies + $nativeDependencyLicenses)) {
        if (-not (Test-Path -LiteralPath $dependencyPath -PathType Leaf)) {
            throw "RAW native dependency or license is missing: $dependencyPath"
        }
    }
    foreach ($license in @("LICENSE.LGPL", "LICENSE.CDDL")) {
        if (-not (Test-Path -LiteralPath (Join-Path $LibRawLicenseDirectory $license) -PathType Leaf)) {
            throw "LibRaw license is missing: $license"
        }
    }

    $packageName = "Lumia-RAW-windows-$Architecture"
    $staging = Join-Path $OutputDirectory $packageName
    $plugin = Join-Path $staging "lumia-plugin-raw"
    $licenses = Join-Path $plugin "licenses"
    $archive = Join-Path $OutputDirectory "$packageName.lumiaplugin"
    $zipArchive = "$archive.zip"
    if (Test-Path -LiteralPath $staging) { Remove-Item -LiteralPath $staging -Recurse -Force }
    if (Test-Path -LiteralPath $archive) { Remove-Item -LiteralPath $archive -Force }
    if (Test-Path -LiteralPath $zipArchive) { Remove-Item -LiteralPath $zipArchive -Force }
    New-Item -ItemType Directory -Force -Path $licenses | Out-Null

    Copy-Item target/release/lumia-plugin-raw.exe $plugin/
    Copy-Item -LiteralPath $BridgeLibrary -Destination $plugin/
    Copy-Item -LiteralPath $LibRawLibrary -Destination $plugin/
    foreach ($nativeDependency in $nativeDependencies) {
        Copy-Item -LiteralPath $nativeDependency -Destination $plugin/
    }
    Copy-Item plugins/lumia-plugin-raw/lumia.plugin.json $plugin/
    Copy-Item plugins/lumia-plugin-raw/THIRD_PARTY_NOTICES.md $plugin/
    Copy-Item (Join-Path $LibRawLicenseDirectory "LICENSE.LGPL") $licenses/
    Copy-Item (Join-Path $LibRawLicenseDirectory "LICENSE.CDDL") $licenses/
    foreach ($nativeDependencyLicense in $nativeDependencyLicenses) {
        Copy-Item -LiteralPath $nativeDependencyLicense -Destination $licenses/
    }

    & "$PSScriptRoot/verify-windows-native-dependencies.ps1" -PluginDirectory $plugin

    $appVersion = (Select-String -Path crates/lumia-app/Cargo.toml -Pattern '^version = "([^"]+)"$').Matches[0].Groups[1].Value
    $pluginApiVersion = (Select-String -Path crates/lumia-plugin-api/src/rpc.rs -Pattern 'PROTOCOL_VERSION: u32 = ([0-9]+)').Matches[0].Groups[1].Value
    node scripts/sign-plugin-package.mjs `
        --root $staging `
        --install-directory lumia-plugin-raw `
        --plugin-id lumia.raw `
        --target-os windows `
        --target-arch $Architecture `
        --minimum-lumia-version $appVersion `
        --plugin-api-version $pluginApiVersion
    if ($LASTEXITCODE -ne 0) { throw "RAW plugin package signing failed" }

    foreach ($required in @(
        "lumia-plugin-raw.exe",
        "lumia.plugin.json",
        "lumia.plugin.sig",
        "THIRD_PARTY_NOTICES.md",
        "zlib1__.dll",
        "libjpeg-9__.dll",
        "licenses/LICENSE.LGPL",
        "licenses/LICENSE.CDDL",
        "licenses/LICENSE.zlib",
        "licenses/LICENSE.libjpeg"
    )) {
        if (-not (Test-Path -LiteralPath (Join-Path $plugin $required))) {
            throw "RAW plugin package is missing $required"
        }
    }
    foreach ($metadata in @("lumia.package.json", "lumia.package.sig")) {
        if (-not (Test-Path -LiteralPath (Join-Path $staging $metadata))) {
            throw "RAW plugin package is missing $metadata"
        }
    }

    Compress-Archive -Path "$staging/*" -DestinationPath $zipArchive
    Move-Item -LiteralPath $zipArchive -Destination $archive
    Write-Host "Created $archive"

    $pluginVersion = (Get-Content plugins/lumia-plugin-raw/lumia.plugin.json -Raw | ConvertFrom-Json).version
    $indexArch = Get-IndexArchitecture $Architecture
    $versioned = Join-Path $OutputDirectory "Lumia-RAW-$pluginVersion-windows-$indexArch.lumiaplugin"
    Copy-Item -LiteralPath $archive -Destination $versioned
    Write-Host "Created $versioned"
} finally {
    $env:RUSTFLAGS = $originalRustFlags
    Pop-Location
}
