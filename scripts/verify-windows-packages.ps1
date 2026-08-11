param(
    [Parameter(Mandatory = $true)]
    [string]$PackageDirectory,
    [switch]$InstallTest
)

$ErrorActionPreference = "Stop"
$packageDirectory = (Resolve-Path $PackageDirectory).Path
$msis = @(Get-ChildItem $packageDirectory -Filter "Lumia-*-x64-*.msi")
$setup = Get-ChildItem $packageDirectory -Filter "Lumia-Setup-*-x64.exe" | Select-Object -First 1
if ($msis.Count -ne 2 -or -not $setup) {
    throw "Expected one Setup EXE and two localized MSI packages"
}

$windowsInstaller = New-Object -ComObject WindowsInstaller.Installer

function Open-Msi([string]$Path) {
    return $script:windowsInstaller.GetType().InvokeMember(
        "OpenDatabase", "InvokeMethod", $null, $script:windowsInstaller, @($Path, 0)
    )
}

function Get-MsiValue($Database, [string]$Query, [int]$Column = 1) {
    try {
        $view = $Database.GetType().InvokeMember(
            "OpenView", "InvokeMethod", $null, $Database, @($Query)
        )
    } catch {
        throw "MSI query failed: $Query`n$($_.Exception.Message)"
    }
    $null = $view.GetType().InvokeMember(
        "Execute", "InvokeMethod", $null, $view, $null
    )
    $record = $view.GetType().InvokeMember(
        "Fetch", "InvokeMethod", $null, $view, $null
    )
    if ($null -eq $record) { return $null }
    return $record.StringData($Column)
}

function Assert-Equal($Actual, $Expected, [string]$Message) {
    if ($Actual -ne $Expected) {
        throw "$Message (expected '$Expected', got '$Actual')"
    }
}

foreach ($msi in $msis) {
    $database = Open-Msi $msi.FullName
    $isChinese = $msi.Name -like "*-zh-CN.msi"
    $expectedLanguage = if ($isChinese) { "2052" } else { "1033" }
    $language = Get-MsiValue $database "SELECT ``Value`` FROM ``Property`` WHERE ``Property``='ProductLanguage'"
    Assert-Equal $language $expectedLanguage "$($msi.Name) ProductLanguage mismatch"

    $allUsers = Get-MsiValue $database "SELECT ``Value`` FROM ``Property`` WHERE ``Property``='ALLUSERS'"
    if ($allUsers) { throw "$($msi.Name) unexpectedly declares ALLUSERS=$allUsers" }
    Assert-Equal (Get-MsiValue $database "SELECT ``Directory_Parent`` FROM ``Directory`` WHERE ``Directory``='LocalProgramsFolder'") "LocalAppDataFolder" "Local programs directory is not per-user"
    Assert-Equal (Get-MsiValue $database "SELECT ``Directory_Parent`` FROM ``Directory`` WHERE ``Directory``='APPLICATIONFOLDER'") "LocalProgramsFolder" "Application directory is not under local Programs"

    $dialogTitle = Get-MsiValue $database "SELECT ``Text`` FROM ``Control`` WHERE ``Dialog_``='InstallOptionsDlg' AND ``Control``='Title'"
    $expectedTitle = if ($isChinese) { "安装选项" } else { "Installation options" }
    Assert-Equal $dialogTitle $expectedTitle "Custom options dialog is not localized"

    Assert-Equal (Get-MsiValue $database "SELECT ``Level`` FROM ``Feature`` WHERE ``Feature``='Complete'") "1" "Main feature level mismatch"
    Assert-Equal (Get-MsiValue $database "SELECT ``Level`` FROM ``Feature`` WHERE ``Feature``='DesktopShortcutFeature'") "2" "Desktop feature must default off"
    Assert-Equal (Get-MsiValue $database "SELECT ``Feature_`` FROM ``FeatureComponents`` WHERE ``Component_``='StartMenuShortcut'") "Complete" "Start menu shortcut must always install"
    Assert-Equal (Get-MsiValue $database "SELECT ``Feature_`` FROM ``FeatureComponents`` WHERE ``Component_``='DesktopShortcut'") "DesktopShortcutFeature" "Desktop shortcut is not isolated in its optional feature"

    foreach ($shortcut in @("ApplicationStartMenuShortcut", "ApplicationDesktopShortcut")) {
        Assert-Equal (Get-MsiValue $database "SELECT ``Target`` FROM ``Shortcut`` WHERE ``Shortcut``='$shortcut'") "[APPLICATIONFOLDER]lumia-app.exe" "$shortcut is not a direct executable shortcut"
        Assert-Equal (Get-MsiValue $database "SELECT ``WkDir`` FROM ``Shortcut`` WHERE ``Shortcut``='$shortcut'") "APPLICATIONFOLDER" "$shortcut working directory mismatch"
        Assert-Equal (Get-MsiValue $database "SELECT ``Icon_`` FROM ``Shortcut`` WHERE ``Shortcut``='$shortcut'") "ApplicationIcon" "$shortcut icon reference is missing"
    }

    $icon = Get-MsiValue $database "SELECT ``Name`` FROM ``Icon`` WHERE ``Name``='ApplicationIcon'"
    Assert-Equal $icon "ApplicationIcon" "MSI icon table is missing Lumia icon"
    $requiredFiles = @{
        LumiaExecutable = "lumia-app.exe"
        SvgThumbnailDll = "lumia_svg_thumbnail.dll"
        PhotoshopPluginExe = "lumia-plugin-photoshop.exe"
        PhotoshopManifestFile = "lumia.plugin.json"
        JpegXlPluginExe = "lumia-plugin-jpeg-xl.exe"
        JpegXlManifestFile = "lumia.plugin.json"
        Jpeg2000PluginExe = "lumia-plugin-jpeg2000.exe"
        Jpeg2000ManifestFile = "lumia.plugin.json"
    }
    foreach ($fileId in $requiredFiles.Keys) {
        $found = Get-MsiValue $database "SELECT ``FileName`` FROM ``File`` WHERE ``File``='$fileId'"
        if (-not $found) { throw "$($msi.Name) is missing $($requiredFiles[$fileId])" }
    }
    $upgradeLanguage = Get-MsiValue $database "SELECT ``Language`` FROM ``Upgrade`` WHERE ``ActionProperty``='LUMIA_UPGRADE_DETECTED'"
    if ($upgradeLanguage) { throw "$($msi.Name) upgrade detection is incorrectly language-specific" }
}

$largestMsi = ($msis | Measure-Object Length -Maximum).Maximum
if ($setup.Length -le $largestMsi) {
    throw "Setup EXE is too small to contain both localized MSI packages"
}

if ($InstallTest) {
    $msi = ($msis | Where-Object Name -Like "*-en-US.msi").FullName
    $installDir = Join-Path $env:LOCALAPPDATA "Programs/Lumia"
    $shell = New-Object -ComObject WScript.Shell
    $startMenuShortcut = Join-Path $shell.SpecialFolders("Programs") "Lumia/Lumia.lnk"
    $desktopShortcut = Join-Path $shell.SpecialFolders("Desktop") "Lumia.lnk"

    function Invoke-Msi([string[]]$Arguments) {
        $process = Start-Process msiexec.exe -Wait -PassThru -WindowStyle Hidden -ArgumentList $Arguments
        if ($process.ExitCode -notin @(0, 3010)) {
            throw "msiexec failed with exit code $($process.ExitCode): $($Arguments -join ' ')"
        }
    }

    function Assert-Shortcut([string]$Path) {
        if (-not (Test-Path -LiteralPath $Path)) { throw "Missing shortcut: $Path" }
        $link = $shell.CreateShortcut($Path)
        Assert-Equal $link.TargetPath (Join-Path $installDir "lumia-app.exe") "Shortcut target mismatch"
        Assert-Equal $link.WorkingDirectory.TrimEnd("\\") $installDir.TrimEnd("\\") "Shortcut working directory mismatch"
        $iconLocation = $link.IconLocation
        if ([string]::IsNullOrWhiteSpace($iconLocation)) {
            throw "Shortcut icon location is empty"
        }
        if ($iconLocation -notmatch '^(?<path>.+),(?<index>-?\d+)$') {
            throw "Shortcut icon location is invalid: $iconLocation"
        }
        Assert-Equal $Matches.index "0" "Shortcut icon index mismatch"
        $iconPath = $Matches.path.Trim('"')
        if (-not (Test-Path -LiteralPath $iconPath -PathType Leaf)) {
            throw "Shortcut icon file is missing: $iconPath"
        }
    }

    Invoke-Msi @("/i", "`"$msi`"", "/qn", "/norestart")
    try {
        if (-not (Test-Path (Join-Path $installDir "lumia-app.exe"))) { throw "Default MSI install is missing lumia-app.exe" }
        if (-not (Test-Path (Join-Path $installDir "lumia_svg_thumbnail.dll"))) { throw "Default MSI install is missing lumia_svg_thumbnail.dll" }
        Assert-Shortcut $startMenuShortcut
        if (Test-Path $desktopShortcut) { throw "Default MSI install unexpectedly created a desktop shortcut" }
    } finally {
        Invoke-Msi @("/x", "`"$msi`"", "/qn", "/norestart")
    }
    if ((Test-Path $installDir) -or (Test-Path $startMenuShortcut) -or (Test-Path $desktopShortcut)) {
        throw "MSI uninstall left files or shortcuts behind"
    }

    Invoke-Msi @("/i", "`"$msi`"", "/qn", "/norestart", "ADDLOCAL=Complete,DesktopShortcutFeature")
    try {
        Assert-Shortcut $startMenuShortcut
        Assert-Shortcut $desktopShortcut
    } finally {
        Invoke-Msi @("/x", "`"$msi`"", "/qn", "/norestart")
    }
    if ((Test-Path $installDir) -or (Test-Path $startMenuShortcut) -or (Test-Path $desktopShortcut)) {
        throw "Optional shortcut MSI uninstall left files or shortcuts behind"
    }
}

Write-Host "Verified Windows packages in $packageDirectory"
