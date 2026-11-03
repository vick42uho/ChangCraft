# Build ChangCraft Windows Installer (MSI)
$ErrorActionPreference = 'Stop'
$Root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$Version = "0.11.0"
$Arch = "x64"
$Dist = Join-Path $Root 'dist\release'
$TargetDir = Join-Path $Root 'target'
$Bin = Join-Path $TargetDir 'release'

New-Item -ItemType Directory -Force -Path $Dist | Out-Null
$Stage = Join-Path $TargetDir "windows-package\$Arch"
Remove-Item -Recurse -Force $Stage -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $Stage | Out-Null

Copy-Item (Join-Path $Bin 'changcraft.exe') $Stage
Copy-Item (Join-Path $Bin 'pdfcraft-cli.exe') $Stage
foreach ($f in 'LICENSE-MIT', 'LICENSE-APACHE', 'NOTICE') {
  Copy-Item (Join-Path $Root $f) $Stage
}
Copy-Item (Join-Path $Root 'assets\app-icon\LICENSE-document.txt') $Stage

# Models folder
$Models = Join-Path $Stage 'models'
New-Item -ItemType Directory -Force -Path $Models | Out-Null
Copy-Item (Join-Path $Root 'assets\models\*') $Models

# WiX Build
$WixExe = "$HOME\.dotnet\tools\wix.exe"
if (-not (Test-Path $WixExe)) {
  $WixExe = "wix"
}

$Msi = Join-Path $Dist "changcraft-$Version-windows-$Arch.msi"
Write-Output "Building MSI: $Msi"

& $WixExe build (Join-Path $PSScriptRoot 'changcraft.wxs') -arch $Arch `
  (Join-Path $PSScriptRoot 'changcraft-ui.wxs') `
  -d "Version=$Version" -d "BinDir=$Stage" -d "ModelsDir=$Models" -d "IconPath=$(Join-Path $Root 'assets\app-icon\pdfcraft.ico')" `
  -o $Msi

if ($LASTEXITCODE -ne 0) {
  throw "WiX build failed with exit code $LASTEXITCODE"
}

Write-Output "Successfully built: $Msi"
Get-Item $Msi | Format-List Length, FullName

# Portable ZIP
$Zip = Join-Path $Dist "changcraft-$Version-windows-$Arch-portable.zip"
Write-Output "Creating Portable ZIP: $Zip"
Set-Content (Join-Path $Stage 'portable.txt') "ChangCraft Portable Mode`r`n"
Remove-Item -Force $Zip -ErrorAction SilentlyContinue
Compress-Archive -Path (Join-Path $Stage '*') -DestinationPath $Zip -CompressionLevel Optimal
Write-Output "Successfully built: $Zip"
Get-Item $Zip | Format-List Length, FullName

