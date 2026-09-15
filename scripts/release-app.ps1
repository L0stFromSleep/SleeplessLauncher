<#
.SYNOPSIS
  Builds the desktop app and publishes it as a GitHub release, mirroring the
  manual-app-build.yml workflow but run locally.

.PARAMETER Tag
  Tag/name for the release (e.g. v1.0.0).

.EXAMPLE
  ./scripts/release-app.ps1 -Tag v1.0.0
#>
param(
    [Parameter(Mandatory = $true)]
    [string]$Tag
)

$ErrorActionPreference = 'Stop'

if (-not (Get-Command gh -ErrorAction SilentlyContinue)) {
    throw "GitHub CLI (gh) is required but was not found on PATH. Install it from https://cli.github.com/ and run 'gh auth login' first."
}

Write-Host "Building app..." -ForegroundColor Cyan
pnpm app:build
if ($LASTEXITCODE -ne 0) {
    throw "pnpm app:build failed with exit code $LASTEXITCODE"
}

$assets = @(
    Get-ChildItem -Path "target/release/bundle/msi/*.msi" -ErrorAction SilentlyContinue
    Get-ChildItem -Path "target/release/bundle/nsis/*-setup.exe" -ErrorAction SilentlyContinue
) | Where-Object { $_ }

if (-not $assets) {
    throw "No installer files found under target/release/bundle/(msi|nsis)/. Did the build succeed?"
}

Write-Host "Creating release '$Tag' with $($assets.Count) asset(s)..." -ForegroundColor Cyan
gh release create $Tag @($assets.FullName) --title $Tag
if ($LASTEXITCODE -ne 0) {
    throw "gh release create failed with exit code $LASTEXITCODE"
}

Write-Host "Release '$Tag' published." -ForegroundColor Green
