#!/usr/bin/env bash
# Builds the desktop app and publishes it as a GitHub release, mirroring the
# manual-app-build.yml workflow but run locally.
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

echo "Building app..."
pnpm app:build

shopt -s nullglob
assets=(target/release/bundle/msi/*.msi target/release/bundle/nsis/*-setup.exe)
shopt -u nullglob

if [ ${#assets[@]} -eq 0 ]; then
  echo "No installer files found under target/release/bundle/{msi,nsis}/. Did the build succeed?" >&2
  exit 1
fi

echo "Creating release '$TAG' with ${#assets[@]} asset(s)..."
gh release create "$TAG" "${assets[@]}" --title "$TAG"

echo "Release '$TAG' published."
