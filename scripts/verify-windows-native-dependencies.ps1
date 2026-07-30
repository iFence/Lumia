param(
    [Parameter(Mandatory = $true)][string]$PluginDirectory,
    [string]$Dumpbin = ""
)

$ErrorActionPreference = "Stop"

function Resolve-DumpbinPath {
    param([string]$RequestedPath)

    if ($RequestedPath) {
        return (Resolve-Path -LiteralPath $RequestedPath).Path
    }

    $command = Get-Command dumpbin.exe -ErrorAction SilentlyContinue
    if ($command) {
        return $command.Source
    }

    $vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
    if (-not (Test-Path -LiteralPath $vswhere -PathType Leaf)) {
        throw "dumpbin.exe is unavailable and vswhere.exe was not found"
    }
    $visualStudio = (& $vswhere -latest -products * `
        -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
        -property installationPath).Trim()
    if ($LASTEXITCODE -ne 0 -or -not $visualStudio) {
        throw "Visual Studio C++ tools are unavailable"
    }
    $toolsRoot = Join-Path $visualStudio "VC\Tools\MSVC"
    $candidates = @(Get-ChildItem -LiteralPath $toolsRoot -Filter dumpbin.exe -Recurse -File)
    $preferred = $candidates |
        Where-Object { $_.FullName -match "\\Hostx64\\x64\\dumpbin\.exe$" } |
        Sort-Object FullName -Descending |
        Select-Object -First 1
    if (-not $preferred) {
        $preferred = $candidates | Sort-Object FullName -Descending | Select-Object -First 1
    }
    if (-not $preferred) {
        throw "dumpbin.exe was not found below $toolsRoot"
    }
    return $preferred.FullName
}

function Get-ImportedDlls {
    param(
        [string]$DumpbinPath,
        [string]$Binary
    )

    $output = & $DumpbinPath /nologo /dependents $Binary 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw "dumpbin failed for $Binary"
    }
    return @($output | ForEach-Object {
        $line = [string]$_
        if ($line -match "^\s+([A-Za-z0-9_.-]+\.dll)\s*$") {
            $Matches[1]
        }
    } | Sort-Object -Unique)
}

$resolvedPluginDirectory = (Resolve-Path -LiteralPath $PluginDirectory).Path
$requiredFiles = @(
    "lumia-plugin-raw.exe",
    "lumia_raw_bridge.dll",
    "raw.dll",
    "zlib1__.dll",
    "libjpeg-9__.dll"
)
foreach ($requiredFile in $requiredFiles) {
    $path = Join-Path $resolvedPluginDirectory $requiredFile
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "RAW plugin native runtime is missing $requiredFile"
    }
}

$dumpbinPath = Resolve-DumpbinPath -RequestedPath $Dumpbin
$bundled = [System.Collections.Generic.HashSet[string]]::new(
    [System.StringComparer]::OrdinalIgnoreCase
)
$binaries = @(Get-ChildItem -LiteralPath $resolvedPluginDirectory -File |
    Where-Object { $_.Extension -in @(".exe", ".dll") } |
    Sort-Object Name)
foreach ($binary in $binaries) {
    [void]$bundled.Add($binary.Name)
}

$systemDirectory = [Environment]::GetFolderPath([Environment+SpecialFolder]::System)
$external = [System.Collections.Generic.List[string]]::new()
foreach ($binary in $binaries) {
    foreach ($dependency in Get-ImportedDlls -DumpbinPath $dumpbinPath -Binary $binary.FullName) {
        if ($bundled.Contains($dependency)) {
            continue
        }
        if ($dependency -match "^(?i:msvcp|vcruntime|concrt|vcomp).+\.dll$") {
            $external.Add("$($binary.Name) -> $dependency (MSVC redistributable)")
            continue
        }
        if ($dependency.StartsWith("api-ms-win-", [System.StringComparison]::OrdinalIgnoreCase)) {
            continue
        }
        if (Test-Path -LiteralPath (Join-Path $systemDirectory $dependency) -PathType Leaf) {
            continue
        }
        $external.Add("$($binary.Name) -> $dependency")
    }
}

if ($external.Count -gt 0) {
    throw "RAW plugin has unpackaged native dependencies: $($external -join '; ')"
}

Write-Host "Verified self-contained Windows dependencies for $($binaries.Count) RAW plugin binaries"
