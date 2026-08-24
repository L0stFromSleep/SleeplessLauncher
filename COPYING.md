# Copying Guidelines

All packages in this repository are licensed under their respective licenses. For more information, refer to the LICENSE file in each package.

For detailed information, consult each package's COPYING.md, LICENSE.txt, or LICENSE file, if available.

## This Fork: Sleepless Launcher

This repository is **Sleepless Launcher**, a personal, independent fork of Modrinth's open-source code, maintained by [L0stFromSleep](https://github.com/L0stFromSleep). It is **not affiliated with, endorsed by, sponsored by, or officially connected to Rinth, Inc. or Modrinth in any way**. "Modrinth" and its logos remain the property of Rinth, Inc. — see the "Modrinth Branding" section below, which still applies in full to this fork.

Any Sleepless Launcher-specific branding (name, icon, wordmark) added on top of Modrinth's original code belongs to this fork and is not covered by Modrinth's own trademarks or asset licenses.

## Modrinth Branding

The use of Modrinth branding elements, including but not limited to the wrench-in-labyrinth logo, the landing image, and any variations thereof, is strictly prohibited without explicit written permission from Rinth, Inc. This includes trademarks, logos, or other branding elements.

> All rights reserved. © 2020-2025 Rinth, Inc.

This includes, but may not be limited to, the following files:

- .idea/icon.svg
- .github/api_cover.png
- .github/app_cover.png
- .github/monorepo_cover.png
- .github/web_cover.png

If you fork this repository, you must remove all Modrinth branding assets from your fork.

> **Status in this fork:** the files listed above, plus several other
> unlisted Modrinth-branded assets found during a release-readiness audit
> (orphaned mascot/logo images, a Modrinth-colored instance icon option, and
> the Modrinth "Rinthbot" mascot image used in the hosting-update-required
> screen), have been removed from `apps/app`, `apps/app-frontend`, and the
> repo root. A handful of Modrinth-branded assets still remain in
> `packages/ui` and `packages/assets` — they aren't reachable from the
> built Sleepless Launcher app (they're only used by Modrinth's web
> frontend, which this fork doesn't ship), but should still be removed or
> replaced before those packages are touched for any other purpose.
