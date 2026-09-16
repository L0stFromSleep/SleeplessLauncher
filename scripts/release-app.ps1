<#
.SYNOPSIS
  Builds a signed, auto-update-capable Windows build of the desktop app and
  publishes it (installer + update manifest) as a GitHub release.

.DESCRIPTION
  Requires:
    - GitHub CLI (gh), authenticated (`gh auth login`)
    - $env:TAURI_SIGNING_PRIVATE_KEY and $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD
      set (generate a keypair once with:
      `pnpm --filter @modrinth/app tauri signer generate -w <path-to-key-file>`,
      store the key and password somewhere safe outside the repo, and put the
      printed public key into apps/app/tauri-release.conf.json's
      plugins.updater.pubkey)

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

if (-not $env:TAURI_SIGNING_PRIVATE_KEY -or -not $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD) {
    throw "TAURI_SIGNING_PRIVATE_KEY and TAURI_SIGNING_PRIVATE_KEY_PASSWORD must be set so the update artifacts can be signed (without this, existing installs can't verify and install updates)."
}

Write-Host "Bumping app version to $Tag..." -ForegroundColor Cyan
node scripts/run.mjs bump-app-version $Tag
if ($LASTEXITCODE -ne 0) {
    throw "bump-app-version failed with exit code $LASTEXITCODE"
}

Write-Host "Building app (release, updater-signed)..." -ForegroundColor Cyan
pnpm --filter @modrinth/app tauri build --config tauri-release.conf.json --features updater
if ($LASTEXITCODE -ne 0) {
    throw "tauri build failed with exit code $LASTEXITCODE"
}

$assets = @(
    Get-ChildItem -Path "target/release/bundle/msi/*.msi" -ErrorAction SilentlyContinue
    Get-ChildItem -Path "target/release/bundle/nsis/*-setup.exe" -ErrorAction SilentlyContinue
) | Where-Object { $_ }

if (-not $assets) {
    throw "No installer files found under target/release/bundle/(msi|nsis)/. Did the build succeed?"
}

$RepoSlug = (gh repo view --json nameWithOwner -q .nameWithOwner)

Write-Host "Generating update manifest for $RepoSlug..." -ForegroundColor Cyan
node scripts/run.mjs generate-update-manifest $Tag $RepoSlug
if ($LASTEXITCODE -ne 0) {
    throw "generate-update-manifest failed with exit code $LASTEXITCODE"
}

Write-Host "Creating release '$Tag' with $($assets.Count) installer(s) + update manifest..." -ForegroundColor Cyan
gh release create $Tag @($assets.FullName) "target/release/bundle/nsis/latest.json" --title $Tag --generate-notes
if ($LASTEXITCODE -ne 0) {
    throw "gh release create failed with exit code $LASTEXITCODE"
}

Write-Host "Release '$Tag' published. Existing installs will pick up this update automatically." -ForegroundColor Green
