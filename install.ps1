# Omasheet installer for Windows.
#
#   irm https://raw.githubusercontent.com/librasteve/omasheet/main/install.ps1 | iex
#
# Puts Omasheet in %LOCALAPPDATA%\Programs\Omasheet (no administrator needed),
# adds it to your PATH, makes a Start menu shortcut, and opens .omx files with
# it. Run it again to update. uninstall with uninstall.ps1 in that folder.
#
# Environment:
#   OMASHEET_REF  the release tag to install, such as v0.1.0 (default: the latest)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$Repo = 'librasteve/omasheet'
$Ref = $env:OMASHEET_REF
$Package = 'omasheet-windows-x86_64.zip'
$Base = if ($Ref) { "https://github.com/$Repo/releases/download/$Ref" } else { "https://github.com/$Repo/releases/latest/download" }
$Dest = Join-Path $env:LOCALAPPDATA 'Programs\Omasheet'

if (-not [Environment]::Is64BitOperatingSystem) { throw 'There is a package for 64-bit Windows only.' }

$Tmp = Join-Path ([IO.Path]::GetTempPath()) ([IO.Path]::GetRandomFileName())
New-Item -ItemType Directory $Tmp | Out-Null
try {
    Write-Host "Installing Omasheet ($(if ($Ref) { $Ref } else { 'latest release' }))"

    Write-Host "  Fetching $Package"
    Invoke-WebRequest "$Base/$Package" -OutFile (Join-Path $Tmp $Package)
    Invoke-WebRequest "$Base/SHA256SUMS" -OutFile (Join-Path $Tmp 'SHA256SUMS')

    Write-Host '  Checking it'
    $Line = Get-Content (Join-Path $Tmp 'SHA256SUMS') | Where-Object { $_ -match " \*?$([regex]::Escape($Package))$" } | Select-Object -First 1
    if (-not $Line) { throw "$Package is not in SHA256SUMS; nothing was installed" }
    $Want = ($Line -split '\s+')[0]
    $Got = (Get-FileHash (Join-Path $Tmp $Package) -Algorithm SHA256).Hash
    if ($Got -ne $Want) { throw "$Package is not what SHA256SUMS says it should be; nothing was installed" }

    Write-Host "  Unpacking to $Dest"
    Get-Process omasheet -ErrorAction SilentlyContinue | Where-Object { $_.Path -like "$Dest\*" } | Stop-Process -Force
    if (Test-Path $Dest) { Remove-Item $Dest -Recurse -Force }
    Expand-Archive (Join-Path $Tmp $Package) -DestinationPath $Dest
    
} finally {
    Remove-Item $Tmp -Recurse -Force -ErrorAction SilentlyContinue
}

$Exe = Join-Path $Dest 'omasheet.exe'

# The omasheet command, for new terminals.
$Path = [Environment]::GetEnvironmentVariable('Path', 'User')
if (($Path -split ';') -notcontains $Dest) {
    [Environment]::SetEnvironmentVariable('Path', (($Path.TrimEnd(';') + ";$Dest").TrimStart(';')), 'User')
}

# The Start menu.
$Menu = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs'
$Shell = New-Object -ComObject WScript.Shell
$Link = $Shell.CreateShortcut((Join-Path $Menu 'Omasheet.lnk'))
$Link.TargetPath = $Exe
$Link.WorkingDirectory = $Dest
$Link.Description = 'Text-native spreadsheet'
$Link.Save()

# .omx opens with Omasheet (per user, so nothing here needs administrator).
$Class = 'HKCU:\Software\Classes\Omasheet.omx'
New-Item -Force "$Class\shell\open\command" -Value "`"$Exe`" `"%1`"" | Out-Null
New-Item -Force $Class -Value 'Omasheet file' | Out-Null
New-Item -Force 'HKCU:\Software\Classes\.omx' -Value 'Omasheet.omx' | Out-Null

# Programs and Features lists it, and removes it.
$Uninstall = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Omasheet'
$Version = (Get-Item $Exe).VersionInfo.ProductVersion
New-Item -Force $Uninstall | Out-Null
Set-ItemProperty $Uninstall DisplayName 'Omasheet'
Set-ItemProperty $Uninstall DisplayVersion "$Version"
Set-ItemProperty $Uninstall Publisher 'Stephen Roe'
Set-ItemProperty $Uninstall InstallLocation $Dest
Set-ItemProperty $Uninstall UninstallString "powershell -NoProfile -ExecutionPolicy Bypass -File `"$Dest\uninstall.ps1`""

Write-Host "Done: $Dest"
Write-Host 'Open it from the Start menu, or run (in a new terminal): omasheet'
