param(
    [string]$OutputDirectory = "target/raw-native",
    [string]$LibRawArchive = "",
    [string]$CMakeArchive = "",
    [string]$WindowsDependencyRoot = "C:/Strawberry"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$resolvedRoot = [System.IO.Path]::GetFullPath($root)
$resolvedOutput = [System.IO.Path]::GetFullPath((Join-Path $root $OutputDirectory))
$targetRoot = [System.IO.Path]::GetFullPath((Join-Path $resolvedRoot "target")) + [System.IO.Path]::DirectorySeparatorChar
if (-not $resolvedOutput.StartsWith($targetRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "RAW native output must stay under the workspace target directory"
}
if (Test-Path -LiteralPath $resolvedOutput) {
    throw "RAW native output already exists: $resolvedOutput"
}

$libRawCommit = "b93f6e45c194f5df9b02a43b1af9a54b4f41f33f"
$cmakeCommit = "eb98e4325aef2ce85d2eb031c2ff18640ca616d3"
$sourceRoot = Join-Path $resolvedOutput "source"
$libRawArchiveSha256 = "B2AF6F35822C6E6AE62D9B5A7E26995DE0550C37C558DDDB4081EC0DAFC0FBCA"
$cmakeArchiveSha256 = "4AF636413E6ECAC21F4DCFA666C8F15C19B9BEDFE4CB2453B85CCC735285581A"
$libRawBuild = Join-Path $resolvedOutput "libraw-build"
$bridgeBuild = Join-Path $resolvedOutput "bridge-build"
$artifacts = Join-Path $resolvedOutput "artifacts"
$artifactLicenses = Join-Path $artifacts "licenses"
$resolvedDependencyRoot = [System.IO.Path]::GetFullPath($WindowsDependencyRoot)
$dependencyFiles = [ordered]@{
    ZlibInclude = Join-Path $resolvedDependencyRoot "c/include/zlib.h"
    ZlibImportLibrary = Join-Path $resolvedDependencyRoot "c/lib/libz.a"
    ZlibRuntime = Join-Path $resolvedDependencyRoot "c/bin/zlib1__.dll"
    ZlibLicense = Join-Path $resolvedDependencyRoot "licenses/libzlib/README"
    JpegInclude = Join-Path $resolvedDependencyRoot "c/include/jpeglib.h"
    JpegImportLibrary = Join-Path $resolvedDependencyRoot "c/lib/libjpeg.a"
    JpegRuntime = Join-Path $resolvedDependencyRoot "c/bin/libjpeg-9__.dll"
    JpegLicense = Join-Path $resolvedDependencyRoot "licenses/libjpeg/README"
}
foreach ($dependency in $dependencyFiles.GetEnumerator()) {
    if (-not (Test-Path -LiteralPath $dependency.Value -PathType Leaf)) {
        throw "Pinned Windows RAW dependency is missing: $($dependency.Key) at $($dependency.Value)"
    }
}
if ((Get-Content -Raw -LiteralPath $dependencyFiles.ZlibLicense) -notmatch "zlib 1\.3\.1") {
    throw "Windows RAW dependency root must provide zlib 1.3.1"
}
if ((Get-Content -Raw -LiteralPath $dependencyFiles.JpegLicense) -notmatch "release 9f") {
    throw "Windows RAW dependency root must provide IJG JPEG 9f"
}
New-Item -ItemType Directory -Force -Path $sourceRoot, $artifacts, $artifactLicenses | Out-Null
function Invoke-PinnedClone {
    param(
        [string]$Name,
        [string]$Repository,
        [string]$Commit,
        [string]$Branch = ""
    )

    for ($attempt = 1; $attempt -le 3; $attempt++) {
        $destination = Join-Path $sourceRoot "$Name-$attempt"
        $cloneArguments = @("clone", "--depth", "1")
        if ($Branch) {
            $cloneArguments += @("--branch", $Branch)
        }
        $cloneArguments += @($Repository, $destination)
        & git @cloneArguments | Out-Host
        if ($LASTEXITCODE -eq 0) {
            & git -C $destination fetch --depth 1 origin $Commit | Out-Host
            if ($LASTEXITCODE -eq 0) {
                & git -C $destination checkout --detach $Commit | Out-Host
                $actualCommit = (& git -C $destination rev-parse HEAD).Trim()
                if ($LASTEXITCODE -eq 0 -and $actualCommit -eq $Commit) {
                    return $destination
                }
            }
        }
        if ($attempt -lt 3) {
            Start-Sleep -Seconds (2 * $attempt)
        }
    }

    throw "$Name clone failed after 3 attempts"
}


Push-Location $root
try {
    if (
        [string]::IsNullOrWhiteSpace($LibRawArchive) -ne
        [string]::IsNullOrWhiteSpace($CMakeArchive)
    ) {
        throw "LibRawArchive and CMakeArchive must be provided together"
    }

    if ($LibRawArchive) {
        $resolvedLibRawArchive = [System.IO.Path]::GetFullPath($LibRawArchive, $resolvedRoot)
        $resolvedCMakeArchive = [System.IO.Path]::GetFullPath($CMakeArchive, $resolvedRoot)
        foreach ($archive in @(
            @{ Path = $resolvedLibRawArchive; Sha256 = $libRawArchiveSha256 },
            @{ Path = $resolvedCMakeArchive; Sha256 = $cmakeArchiveSha256 }
        )) {
            if (-not (Test-Path -LiteralPath $archive.Path -PathType Leaf)) {
                throw "Pinned source archive is missing: $($archive.Path)"
            }
            $actualHash = (Get-FileHash -LiteralPath $archive.Path -Algorithm SHA256).Hash
            if ($actualHash -ne $archive.Sha256) {
                throw "Pinned source archive checksum mismatch: $($archive.Path)"
            }
            Expand-Archive -LiteralPath $archive.Path -DestinationPath $sourceRoot
        }
        $libRawSource = Join-Path $sourceRoot "LibRaw-$libRawCommit"
        $cmakeSource = Join-Path $sourceRoot "LibRaw-cmake-$cmakeCommit"
        foreach ($directory in @($libRawSource, $cmakeSource)) {
            if (-not (Test-Path -LiteralPath $directory -PathType Container)) {
                throw "Pinned source archive has an unexpected root directory: $directory"
            }
        }
    } else {
        $libRawSource = Invoke-PinnedClone -Name "LibRaw" -Repository "https://github.com/LibRaw/LibRaw.git" `
            -Commit $libRawCommit -Branch "0.22.2"
        $cmakeSource = Invoke-PinnedClone -Name "LibRaw-cmake" `
            -Repository "https://github.com/LibRaw/LibRaw-cmake.git" -Commit $cmakeCommit
    }

    cmake -A x64 -S $cmakeSource -B $libRawBuild `
        "-DCMAKE_CXX_FLAGS=/DLIBRAW_WIN32_UNICODEPATHS /EHsc" `
        "-DCMAKE_POLICY_DEFAULT_CMP0091=NEW" `
        "-DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded" `
        "-DLIBRAW_PATH=$libRawSource" `
        "-DZLIB_INCLUDE_DIR=$(Split-Path -Parent $dependencyFiles.ZlibInclude)" `
        "-DZLIB_LIBRARY=$($dependencyFiles.ZlibImportLibrary)" `
        "-DZLIB_LIBRARY_RELEASE=$($dependencyFiles.ZlibImportLibrary)" `
        "-DJPEG_INCLUDE_DIR=$(Split-Path -Parent $dependencyFiles.JpegInclude)" `
        "-DJPEG_LIBRARY=$($dependencyFiles.JpegImportLibrary)" `
        "-DJPEG_LIBRARY_RELEASE=$($dependencyFiles.JpegImportLibrary)" `
        -DBUILD_SHARED_LIBS=ON `
        -DENABLE_EXAMPLES=OFF `
        -DENABLE_OPENMP=OFF `
        -DENABLE_LCMS=OFF `
        -DENABLE_JASPER=OFF `
        -DENABLE_X3FTOOLS=ON
    if ($LASTEXITCODE -ne 0) { throw "LibRaw configuration failed" }
    cmake --build $libRawBuild --config Release --target raw
    if ($LASTEXITCODE -ne 0) { throw "LibRaw build failed" }

    $rawImportLibrary = @(
        (Join-Path $libRawBuild "Release/raw.lib"),
        (Join-Path $libRawBuild "raw.lib"),
        (Join-Path $libRawBuild "Release/libraw.dll.a"),
        (Join-Path $libRawBuild "libraw.dll.a")
    ) | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } | Select-Object -First 1
    $rawRuntime = @(
        (Join-Path $libRawBuild "Release/raw.dll"),
        (Join-Path $libRawBuild "raw.dll"),
        (Join-Path $libRawBuild "Release/libraw.dll"),
        (Join-Path $libRawBuild "libraw.dll")
    ) | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } | Select-Object -First 1
    if (-not $rawImportLibrary -or -not $rawRuntime) {
        throw "LibRaw build outputs are missing"
    }
    cmake -A x64 -S plugins/lumia-plugin-raw/native -B $bridgeBuild `
        "-DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded" `
        "-DLIBRAW_INCLUDE_DIR=$libRawSource" `
        "-DLIBRAW_LIBRARY=$rawImportLibrary"
    if ($LASTEXITCODE -ne 0) { throw "RAW bridge configuration failed" }
    cmake --build $bridgeBuild --config Release
    if ($LASTEXITCODE -ne 0) { throw "RAW bridge build failed" }

    $bridgeRuntime = @(
        (Join-Path $bridgeBuild "Release/lumia_raw_bridge.dll"),
        (Join-Path $bridgeBuild "lumia_raw_bridge.dll")
    ) | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } | Select-Object -First 1
    foreach ($file in @($rawRuntime, $bridgeRuntime)) {
        if (-not (Test-Path -LiteralPath $file -PathType Leaf)) {
            throw "RAW native build output is missing: $file"
        }
    }
    $packagedRaw = Join-Path $artifacts ([System.IO.Path]::GetFileName($rawRuntime))
    $packagedBridge = Join-Path $artifacts "lumia_raw_bridge.dll"
    Copy-Item -LiteralPath $rawRuntime -Destination $packagedRaw
    Copy-Item -LiteralPath $bridgeRuntime -Destination $packagedBridge
    Copy-Item -LiteralPath $dependencyFiles.ZlibRuntime -Destination $artifacts
    Copy-Item -LiteralPath $dependencyFiles.JpegRuntime -Destination $artifacts
    Copy-Item -LiteralPath $dependencyFiles.ZlibLicense `
        -Destination (Join-Path $artifactLicenses "LICENSE.zlib")
    Copy-Item -LiteralPath $dependencyFiles.JpegLicense `
        -Destination (Join-Path $artifactLicenses "LICENSE.libjpeg")

    if ($env:GITHUB_OUTPUT) {
        "bridge=$packagedBridge" | Out-File -FilePath $env:GITHUB_OUTPUT -Append -Encoding utf8
        "libraw=$packagedRaw" | Out-File -FilePath $env:GITHUB_OUTPUT -Append -Encoding utf8
        "licenses=$libRawSource" | Out-File -FilePath $env:GITHUB_OUTPUT -Append -Encoding utf8
        "runtime_directory=$artifacts" | Out-File -FilePath $env:GITHUB_OUTPUT -Append -Encoding utf8
    }
    Write-Host "RAW bridge: $packagedBridge"
    Write-Host "LibRaw runtime: $packagedRaw"
    Write-Host "Native dependency directory: $artifacts"
} finally {
    Pop-Location
}
