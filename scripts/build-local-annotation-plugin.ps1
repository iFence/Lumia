#requires -Version 7.0

[CmdletBinding()]
param(
    [string]$PrivateKeyPath = (
        Join-Path `
            ([Environment]::GetFolderPath([Environment+SpecialFolder]::UserProfile)) `
            "apps\LumiaSecrets\lumia-plugin-signing-key.pem"
    ),
    [string]$OutputDirectory = "target"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$rootPath = [System.IO.Path]::GetFullPath($root)
$privateKey = (Resolve-Path -LiteralPath $PrivateKeyPath).Path
$rootPrefix = $rootPath.TrimEnd(
    [System.IO.Path]::DirectorySeparatorChar,
    [System.IO.Path]::AltDirectorySeparatorChar
) + [System.IO.Path]::DirectorySeparatorChar

if ($privateKey.StartsWith($rootPrefix, [StringComparison]::OrdinalIgnoreCase)) {
    throw "The plugin signing key must be stored outside the Lumia repository"
}

$signingKey = Get-Content -Raw -LiteralPath $privateKey
if (-not $signingKey.Contains("-----BEGIN PRIVATE KEY-----")) {
    throw "The plugin signing key must be an Ed25519 PKCS#8 PEM private key"
}

$previousSigningKey = [Environment]::GetEnvironmentVariable(
    "LUMIA_PLUGIN_SIGNING_KEY_PEM",
    [EnvironmentVariableTarget]::Process
)

Push-Location $rootPath
try {
    $outputPath = if ([System.IO.Path]::IsPathRooted($OutputDirectory)) {
        [System.IO.Path]::GetFullPath($OutputDirectory)
    } else {
        [System.IO.Path]::GetFullPath((Join-Path $rootPath $OutputDirectory))
    }
    $packagePath = Join-Path $outputPath "Lumia-Annotation-windows-x64.lumiaplugin"

    $env:LUMIA_PLUGIN_SIGNING_KEY_PEM = $signingKey

    Write-Host "Building the Lumia package verifier..."
    cargo build --release -p lumia-app
    if ($LASTEXITCODE -ne 0) {
        throw "Lumia release build failed"
    }

    Write-Host "Building and signing the Annotation plugin package..."
    & "$PSScriptRoot/package-annotation-plugin.ps1" `
        -OutputDirectory $outputPath `
        -Architecture "x64"

    Write-Host "Verifying the generated plugin package..."
    & "$PSScriptRoot/verify-plugin-package.ps1" `
        -Package $packagePath

    Write-Host ""
    Write-Host "Local Annotation plugin package is ready:"
    Write-Host "  $packagePath"
} finally {
    if ($null -eq $previousSigningKey) {
        Remove-Item Env:\LUMIA_PLUGIN_SIGNING_KEY_PEM `
            -ErrorAction SilentlyContinue
    } else {
        [Environment]::SetEnvironmentVariable(
            "LUMIA_PLUGIN_SIGNING_KEY_PEM",
            $previousSigningKey,
            [EnvironmentVariableTarget]::Process
        )
    }
    $signingKey = $null
    Pop-Location
}
