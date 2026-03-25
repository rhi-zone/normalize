# Normalize CLI installer for Windows
# Usage: irm https://raw.githubusercontent.com/rhi-zone/normalize/master/install.ps1 | iex
# Version pinning: $env:NORMALIZE_VERSION = "0.2.0"; irm ... | iex

$ErrorActionPreference = "Stop"

$Repo = "rhi-zone/normalize"
$InstallDir = "$env:LOCALAPPDATA\Programs\normalize"

# Create install directory
if (-not (Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
}

# Resolve version
if ($env:NORMALIZE_VERSION) {
    $Version = $env:NORMALIZE_VERSION.TrimStart("v")
    $Tag = "v$Version"
} else {
    Write-Host "Fetching latest release..."
    $Release = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest"
    $Tag = $Release.tag_name
    $Version = $Tag.TrimStart("v")
}

Write-Host "Installing normalize $Tag..."

# Download archive and checksums
$Asset = "normalize-x86_64-pc-windows-msvc.zip"
$BaseUrl = "https://github.com/$Repo/releases/download/$Tag"
$ZipPath = "$env:TEMP\normalize-$Version.zip"
$SumsPath = "$env:TEMP\normalize-$Version-SHA256SUMS.txt"

Invoke-WebRequest -Uri "$BaseUrl/$Asset" -OutFile $ZipPath
Invoke-WebRequest -Uri "$BaseUrl/SHA256SUMS.txt" -OutFile $SumsPath

# Verify checksum
$SumsContent = Get-Content $SumsPath
$ExpectedLine = $SumsContent | Where-Object { $_ -match [regex]::Escape($Asset) }
if (-not $ExpectedLine) {
    Write-Error "No checksum found for $Asset in SHA256SUMS.txt"
    exit 1
}
$Expected = ($ExpectedLine -split '\s+')[0].ToLower()

$ActualHash = (Get-FileHash -Path $ZipPath -Algorithm SHA256).Hash.ToLower()
if ($ActualHash -ne $Expected) {
    Write-Error "Checksum mismatch!`n  Expected: $Expected`n  Got:      $ActualHash"
    exit 1
}
Write-Host "Checksum verified."

# Extract
Expand-Archive -Path $ZipPath -DestinationPath $InstallDir -Force
Remove-Item $ZipPath
Remove-Item $SumsPath

# Add to PATH if not already there
$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($UserPath -notlike "*$InstallDir*") {
    Write-Host "Adding $InstallDir to PATH..."
    [Environment]::SetEnvironmentVariable("Path", "$UserPath;$InstallDir", "User")
    $env:Path = "$env:Path;$InstallDir"
}

Write-Host ""
Write-Host "Installed normalize $Tag to $InstallDir\normalize.exe"

# Verify
try {
    $InstalledVersion = & "$InstallDir\normalize.exe" --version 2>&1
    Write-Host $InstalledVersion
} catch {
    Write-Host "Run 'normalize --version' to verify installation (restart terminal if needed)."
}
