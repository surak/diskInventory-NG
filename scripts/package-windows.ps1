# Builds target\release\bundle\DiskInventoryNG-<version>-windows-x64.zip
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

$version = (Select-String -Path Cargo.toml -Pattern '^version = "(.*)"').Matches[0].Groups[1].Value
cargo build --release
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

$name = "DiskInventoryNG-$version-windows-x64"
$stage = Join-Path ([IO.Path]::GetTempPath()) $name
Remove-Item -Recurse -Force $stage -ErrorAction SilentlyContinue
New-Item -ItemType Directory $stage | Out-Null
Copy-Item target\release\disk-inventory-ng.exe (Join-Path $stage "Disk Inventory NG.exe")
Copy-Item README.md, LICENSE $stage

New-Item -ItemType Directory -Force target\release\bundle | Out-Null
$zip = "target\release\bundle\$name.zip"
Remove-Item $zip -ErrorAction SilentlyContinue
Compress-Archive -Path $stage -DestinationPath $zip
Write-Host "Built $zip"
