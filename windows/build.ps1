# Build the Windows package, on Windows with Rust (MSVC) and Qt 6 (its bin
# directory on PATH, so that qmake and windeployqt are found). Leaves
# windows\dist\omasheet-windows-x86_64.zip, which install.ps1 fetches.

$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')

# --locked: see install.sh on the yanked crate.
cargo build --release --locked --package omasheet-ui
if ($LASTEXITCODE) { throw 'cargo build failed' }

$Dist = 'windows\dist'
$Stage = Join-Path $Dist 'omasheet'
Remove-Item $Dist -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory $Stage | Out-Null
Copy-Item target\release\omasheet.exe $Stage
Copy-Item LICENSE $Stage
Copy-Item windows\uninstall.ps1 $Stage

# Copy in the Qt it was built with: its DLLs, QML modules and plugins.
windeployqt --release --qmldir omasheet-ui\qml (Join-Path $Stage 'omasheet.exe')
if ($LASTEXITCODE) { throw 'windeployqt failed' }

Compress-Archive -Path "$Stage\*" -DestinationPath (Join-Path $Dist 'omasheet-windows-x86_64.zip')
Get-Item (Join-Path $Dist '*.zip')
