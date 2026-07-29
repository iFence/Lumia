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

    $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $executablePath
    $startInfo.UseShellExecute = $false
    $startInfo.ArgumentList.Add("--verify-plugin-package")
    $startInfo.ArgumentList.Add($packagePath)

    $process = [System.Diagnostics.Process]::Start($startInfo)
    if ($null -eq $process) {
        throw "Failed to start Lumia package verifier"
    }
    try {
        $process.WaitForExit()
        if ($process.ExitCode -ne 0) {
            throw "Lumia rejected plugin package $packagePath (exit code $($process.ExitCode))"
        }
    } finally {
        $process.Dispose()
    }
    Write-Host "Verified plugin package $packagePath"
} finally {
    Pop-Location
}
