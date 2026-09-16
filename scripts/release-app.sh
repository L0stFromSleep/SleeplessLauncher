#!/usr/bin/env bash
# Builds a signed, auto-update-capable Windows build of the desktop app and
# publishes it (installer + update manifest) as a GitHub release.
#
# Requires:
#   - GitHub CLI (gh), authenticated (`gh auth login`)
#   - TAURI_SIGNING_PRIVATE_KEY and TAURI_SIGNING_PRIVATE_KEY_PASSWORD set in
#     the environment (generate a keypair once with:
#     `pnpm --filter @modrinth/app tauri signer generate -w <path-to-key-file>`,
#     store the key and password somewhere safe outside the repo, and put the
#     printed public key into apps/app/tauri-release.conf.json's
#     plugins.updater.pubkey)
#
# Usage: ./scripts/release-app.sh v1.0.0

set -euo pipefail

TAG="${1:-}"
if [ -z "$TAG" ]; then
  echo "Usage: $0 <tag>" >&2
  echo "Example: $0 v1.0.0" >&2
  exit 1
fi

if ! command -v gh >/dev/null 2>&1; then
  echo "GitHub CLI (gh) is required but was not found on PATH. Install it from https://cli.github.com/ and run 'gh auth login' first." >&2
  exit 1
fi

if [ -z "${TAURI_SIGNING_PRIVATE_KEY:-}" ] || [ -z "${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}" ]; then
  echo "TAURI_SIGNING_PRIVATE_KEY and TAURI_SIGNING_PRIVATE_KEY_PASSWORD must be set so the update" >&2
  echo "artifacts can be signed (without this, existing installs can't verify and install updates)." >&2
  exit 1
fi

echo "Bumping app version to $TAG..."
node scripts/run.mjs bump-app-version "$TAG"

echo "Building app (release, updater-signed)..."
pnpm --filter @modrinth/app tauri build --config tauri-release.conf.json --features updater

shopt -s nullglob
assets=(target/release/bundle/msi/*.msi target/release/bundle/nsis/*-setup.exe)
shopt -u nullglob

if [ ${#assets[@]} -eq 0 ]; then
  echo "No installer files found under target/release/bundle/{msi,nsis}/. Did the build succeed?" >&2
  exit 1
fi

REPO_SLUG="$(gh repo view --json nameWithOwner -q .nameWithOwner)"

echo "Generating update manifest for $REPO_SLUG..."
node scripts/run.mjs generate-update-manifest "$TAG" "$REPO_SLUG"

echo "Creating release '$TAG' with ${#assets[@]} installer(s) + update manifest..."
gh release create "$TAG" "${assets[@]}" target/release/bundle/nsis/latest.json --title "$TAG" --generate-notes

echo "Release '$TAG' published. Existing installs will pick up this update automatically."
