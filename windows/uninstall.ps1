# Removes Omasheet, as install.ps1 put it. Files you made are not touched.

$ErrorActionPreference = 'Continue'
$Dest = Join-Path $env:LOCALAPPDATA 'Programs\Omasheet'

Get-Process omasheet -ErrorAction SilentlyContinue | Where-Object { $_.Path -like "$Dest\*" } | Stop-Process -Force

$Path = [Environment]::GetEnvironmentVariable('Path', 'User')
$Kept = ($Path -split ';' | Where-Object { $_ -and $_ -ne $Dest }) -join ';'
[Environment]::SetEnvironmentVariable('Path', $Kept, 'User')

Remove-Item (Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\Omasheet.lnk') -ErrorAction SilentlyContinue
Remove-Item 'HKCU:\Software\Classes\Omasheet.omx' -Recurse -ErrorAction SilentlyContinue
Remove-Item 'HKCU:\Software\Classes\.omx' -Recurse -ErrorAction SilentlyContinue
Remove-Item 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Omasheet' -Recurse -ErrorAction SilentlyContinue

# This script runs from inside Dest: leave it for last, from a copy.
$Self = Join-Path ([IO.Path]::GetTempPath()) 'omasheet-rm.cmd'
Set-Content $Self "@echo off`r`nping -n 2 127.0.0.1 >nul`r`nrmdir /s /q `"$Dest`"`r`ndel `"%~f0`""
Start-Process cmd.exe -ArgumentList '/c', $Self -WindowStyle Hidden
Write-Host 'Omasheet is removed.'
