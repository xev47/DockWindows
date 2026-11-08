param(
    [string]$Version
)

$ErrorActionPreference = "Stop"

$installerDir = $PSScriptRoot
$rustDockDir = Split-Path $installerDir -Parent
$cargoManifest = Join-Path $rustDockDir "Cargo.toml"

if ([string]::IsNullOrWhiteSpace($Version)) {
    $manifest = Get-Content $cargoManifest -Raw
    if ($manifest -notmatch '(?m)^version\s*=\s*"(\d+\.\d+\.\d+)"\s*$') {
        throw "Could not read a numeric package version from Cargo.toml."
    }
    $Version = $Matches[1]
}
if ($Version -notmatch '^\d+\.\d+\.\d+$') {
    throw "Version must use the MSI format major.minor.patch, for example 1.2.3."
}

$wixCommand = Get-Command wix -ErrorAction SilentlyContinue
$wixExe = if ($wixCommand) {
    $wixCommand.Source
} else {
    Join-Path $env:USERPROFILE ".dotnet\tools\wix.exe"
}
if (-not (Test-Path $wixExe)) {
    throw "WiX Toolset v4 is required. Install it with: dotnet tool install --global wix --version 4.0.5"
}

# Le dock doit être arrêté pour que le binaire Release puisse être remplacé.
$running = Get-Process dock-windows -ErrorAction SilentlyContinue
if ($running) {
    Write-Host "Stopping running Dock Windows instance..."
    $running | Stop-Process -Force
    Start-Sleep -Milliseconds 500
}

Write-Host "Building Dock Windows (Release, x64)..."
cargo build --release --manifest-path $cargoManifest
if ($LASTEXITCODE -ne 0) { throw "cargo build failed." }

$outputDir = Join-Path $rustDockDir "target\installer"
New-Item -ItemType Directory -Force -Path $outputDir | Out-Null
$msi = Join-Path $outputDir "DockWindows-$Version-x64.msi"

Write-Host "Building MSI $Version..."
Push-Location $installerDir
try {
    & $wixExe build "Installer.wxs" -o $msi -arch x64 `
        -ext WixToolset.UI.wixext/4.0.5 -ext WixToolset.Util.wixext/4.0.5 `
        -d "ProductVersion=$Version"
    if ($LASTEXITCODE -ne 0) { throw "wix build failed." }
}
finally {
    Pop-Location
}

$hash = (Get-FileHash -Algorithm SHA256 -Path $msi).Hash.ToLowerInvariant()
$checksum = "$msi.sha256"
"$hash  $(Split-Path $msi -Leaf)" | Set-Content -Path $checksum -Encoding ascii

Write-Host "Built: $msi"
Write-Host "SHA256: $checksum"
