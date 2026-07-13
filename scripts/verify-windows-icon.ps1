param(
    [string]$Path = "crates/lumia-app/resources/icon.ico"
)

$ErrorActionPreference = "Stop"
$expectedSizes = @(16, 24, 32, 48, 64, 128, 256)
$bytes = [System.IO.File]::ReadAllBytes((Resolve-Path $Path))

if ($bytes.Length -lt 6) {
    throw "ICO header is truncated"
}
$reserved = [BitConverter]::ToUInt16($bytes, 0)
$type = [BitConverter]::ToUInt16($bytes, 2)
$count = [BitConverter]::ToUInt16($bytes, 4)
if ($reserved -ne 0 -or $type -ne 1) {
    throw "File is not a Windows icon container"
}
if ($count -ne $expectedSizes.Count) {
    throw "Expected $($expectedSizes.Count) icon frames, found $count"
}
if ($bytes.Length -lt 6 + (16 * $count)) {
    throw "ICO directory is truncated"
}

$actualSizes = @()
for ($index = 0; $index -lt $count; $index++) {
    $entry = 6 + (16 * $index)
    $width = if ($bytes[$entry] -eq 0) { 256 } else { [int]$bytes[$entry] }
    $height = if ($bytes[$entry + 1] -eq 0) { 256 } else { [int]$bytes[$entry + 1] }
    if ($width -ne $height) {
        throw "Frame $index is not square: ${width}x${height}"
    }
    $length = [BitConverter]::ToUInt32($bytes, $entry + 8)
    $offset = [BitConverter]::ToUInt32($bytes, $entry + 12)
    if ($length -eq 0 -or ([uint64]$offset + [uint64]$length) -gt $bytes.Length) {
        throw "Frame $index points outside the ICO file"
    }
    $actualSizes += $width
}

$actualSizes = @($actualSizes | Sort-Object)
if (Compare-Object $expectedSizes $actualSizes) {
    throw "Unexpected icon sizes: $($actualSizes -join ', ')"
}

Write-Host "Verified Lumia ICO frames: $($actualSizes -join ', ')"
