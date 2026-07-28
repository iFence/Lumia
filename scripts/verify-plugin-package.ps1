param(
    [Parameter(Mandatory = $true)]
    [string]$Package,
    [string]$LumiaExecutable = "target/release/lumia-app.exe"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Push-Location $root
try {
    $packagePath = (Resolve-Path $Package).Path
    $executablePath = (Resolve-Path $LumiaExecutable).Path
    & $executablePath --verify-plugin-package $packagePath
    if ($LASTEXITCODE -ne 0) {
        throw "Lumia rejected plugin package $packagePath"
    }
    Write-Host "Verified plugin package $packagePath"
} finally {
    Pop-Location
}
