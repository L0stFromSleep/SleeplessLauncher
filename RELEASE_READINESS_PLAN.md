# Release Readiness Plan — Sleepless Launcher

This document tracks what's left before this repo (a personal fork of
[`modrinth/code`](https://github.com/modrinth/code), adding a CurseForge
content provider alongside Modrinth in the Explore/search UI) is safe to
release publicly. App-level rebranding — Tauri product name, bundle
identifier, deep-link scheme, application icon set, and the shared UI logo —
is already done. This plan covers what's left, investigated against the
actual repo state (not assumed), with file-level detail so each item can be
executed without re-investigating.

**Status: items 1–5 below have all been executed** (branding assets removed,
CurseForge rate limiting implemented, live-backend copy/support-link audit
applied and en-US locale regenerated, README/COPYING rewritten, git history
secret-scanned). Each section below is left as a record of what was done,
plus a couple of items still requiring the repo owner's own follow-up
(noted inline).

**Priority order and why**, expanded on at the end of each section:

1. Branding assets — cheap, mechanical, blocks nothing else.
2. CurseForge rate limiting — no legal/impersonation risk, but worth doing
   before real release traffic risks tripping CurseForge's undisclosed
   limits and getting the app's API key flagged.
3. Live-backend / official-client branding audit — the real external-facing
   risk item (real user accounts, real API traffic, support links currently
   misdirecting users). Already fully investigated below.
4. README.md / COPYING.md rewrite — partly depends on decisions made in
   items 1 and 3 (which assets survive, what the fork's real support channel
   is), and isn't runtime-facing, so it's sequenced after them.
5. Git history secret scan — a point-in-time gate, best run immediately
   before flipping the repo to public so it also catches anything introduced
   by the commits from items 1–4.

---

## 1. Remaining Modrinth branding assets

**Status: done.** All files below were removed (via `git rm`). Also found
and removed mid-execution, beyond the original sweep: the Modrinth
"Rinthbot" mascot image (`packages/assets/branding/rinthbot/excited.webp`,
imported as `ExcitedRinthbot` from `@modrinth/assets`) was actively
rendered in `HostingUpdateRequired.vue` — removed along with its usage.
`wrench-rinth.png`'s icon-editor catalog entry was removed outright (repo
owner's choice, no replacement asset supplied).

### Current state

`COPYING.md` at the repo root states, in part:

> "The use of Modrinth branding elements, including but not limited to the
> wrench-in-labyrinth logo, the landing image, and any variations thereof,
> is strictly prohibited without explicit written permission from Rinth,
> Inc. ... If you fork this repository, you must remove all Modrinth
> branding assets from your fork."

It explicitly names `.idea/icon.svg` and `.github/api_cover.png`,
`app_cover.png`, `monorepo_cover.png`, `web_cover.png` (the real paths are
under `.github/assets/`, not `.github/` directly — minor path drift in the
doc, the files themselves are confirmed present). Per-package `COPYING.md`
files (`packages/ui`, `packages/assets`, `apps/frontend`, `apps/docs`,
`packages/blog`) list additional, more specific assets under the same rule.

A broader sweep beyond the four named files found:

| Path | What it is | Referenced in code? | Action |
|---|---|---|---|
| `.idea/icon.svg` | Modrinth wrench-in-labyrinth mark, IDE project icon | No (IDE metadata only) | Delete |
| `.github/assets/monorepo_cover.png` | "Modrinth Monorepo" README banner | `README.md:1` | Delete, remove/replace the reference (see §2) |
| `.github/assets/app_cover.png` | "Modrinth App" banner | `apps/app/README.md` | Delete + edit that README |
| `.github/assets/web_cover.png` | Frontend cover banner | `apps/frontend/README.md` | Delete + edit that README |
| `.github/assets/api_cover.png` | labrinth (backend) banner | `apps/labrinth/README.md` | Delete + edit that README |
| `apps/app-frontend/src/assets/modrinth_app.svg` | Modrinth App logo variant | Unreferenced (orphaned) | Delete outright |
| `apps/app-frontend/src/assets/sad-modrinth-bot.webp` | Modrinth mascot illustration | Unreferenced | Delete outright |
| `apps/app-frontend/src/assets/welcome/modrinth-social-icon.png` | Modrinth social/OG icon | Unreferenced | Delete outright |
| `apps/app-frontend/src/assets/instance-icons/wrench-rinth.png` | Modrinth-colored wrench, selectable instance icon | Yes — `.../icon-editor-modal/editor-catalog.ts` | Needs a replacement asset, or remove the catalog entry — **blocked on repo owner**, see below |
| `apps/app/icons/apple.icon/Assets/Modrinth logo.svg` | File literally named "Modrinth logo.svg", inside an unused iconset subfolder | Not referenced by `tauri.conf.json` or any build config | Delete outright — missed by the earlier icon-replacement pass |
| `logo.png` (repo root) | Byte-identical duplicate of the already-rebranded Sleepless Launcher logo | Unreferenced | Not Modrinth branding — harmless clutter, optional delete |
| `apps/app/icons/*` (16 files: `128x128.png`, `icon.icns`, `icon.ico`, `Square*.png`, `StoreLogo.png`, `favicon.ico`, `icon.png`) | Actual Tauri bundle icons, referenced by `tauri.conf.json` | Yes, actively used | **Already replaced** — confirmed all 16 changed in commit `7730314` ("Holy Progress Batman") |

Lower priority — present in packages the launcher doesn't actually pull
from at runtime, so not blocking a launcher-only release, but still shipped
source in this repo:

- `packages/ui/src/components/brand/TextLogo.vue`, `AnimatedLogo.vue` —
  full Modrinth wordmark SVG paths, explicitly named in
  `packages/ui/COPYING.md`. Only reachable from `apps/frontend` (the web
  app) via `packages/ui/src/components/billing/PurchaseModal.vue`, not from
  `apps/app-frontend`/`apps/app` (the launcher).
- `packages/ui/src/components/servers/ModrinthServersIcon.vue`,
  `AutoBrandIcon.vue` — same situation, unreferenced from the launcher.
- `packages/assets/branding/logo.svg`, `modrinth-plus.svg` — full logo /
  Modrinth Plus logo, not imported by the launcher.
- `packages/ui/src/assets/servers/server-list-empty/modrinth-smp.png` —
  server-promo image, not used by the launcher.

Out of scope entirely for a launcher-only release (still Modrinth-branded
per their own `COPYING.md` files, but belong to the web app / docs / blog,
not the desktop app): `apps/frontend/src/assets/images/logo.svg`,
`apps/frontend/src/public/favicon*.ico`,
`apps/docs/src/assets/{dark,light}-logo.svg`, `apps/labrinth/assets/logo.svg`,
`packages/blog/articles/*modrinth*`, `apps/frontend/src/components/brand/*`.

Two generic icons were checked and are **not** branding, despite adjacent
naming: `packages/assets/icons/tags/categories/modrinth.svg` (a generic pin
glyph) and `packages/assets/icons/wrench.svg` (a generic wrench glyph) — no
action needed on these.

### Proposed fix

Delete the unreferenced/orphaned files outright (`.idea/icon.svg`, the
three orphaned `app-frontend/src/assets/*` files, the `apple.icon` leftover,
optionally `logo.png`). For the four cover PNGs, delete them as part of the
README rewrite pass (§2) that removes/replaces their references in the same
commit, so nothing is left link-broken. For `wrench-rinth.png`, either swap
in a non-Modrinth-branded replacement image at the same catalog entry, or
delete the catalog entry from `editor-catalog.ts` if no replacement is
supplied. Treat the lower-priority `packages/ui`/`packages/assets` items as
a fast-follow rather than a blocker, since they don't ship in the built
launcher binary.

### Blocked on repo owner

- A replacement image for `wrench-rinth.png`'s icon-editor catalog slot, if
  keeping that slot is wanted (otherwise just remove the entry — a decision
  call, not something to guess at).

---

## 2. README.md and COPYING.md rewrite

**Status: done.** Root `README.md` rewritten as attribution-based fork
description; `COPYING.md` got a new "This Fork: Sleepless Launcher" section
plus a status note on which branding assets have been removed vs. still
remain in out-of-scope packages. Per-app `README.md` banner references
(`apps/app`, `apps/frontend`, `apps/labrinth`) were fixed alongside the
asset deletions in §1.

### Current state

`README.md` (39 lines) is written entirely in first person as though this
repository IS Modrinth's own monorepo:

- Line 1: Modrinth's own cover image (`.github/assets/monorepo_cover.png`).
- Lines 3–8: five shields.io/loctopus badges pointing at the real
  `modrinth/code` GitHub repo (issues, PRs, contributors, commit activity,
  last commit) — these reflect upstream's stats, not this fork's.
- Line 10: heading `## Modrinth Monorepo`.
- Line 12: "Welcome to the Modrinth Monorepo, the primary codebase for the
  Modrinth web interface and app" + another `modrinth/code` stats badge —
  directly claims to be Modrinth's primary codebase.
- Line 14: sends users to `modrinth.com/app` to "download the latest
  release of the app" — actively misleading for a fork with its own builds.
- Lines 20–21, 25: dev-docs and contributing-guidelines links to
  `docs.modrinth.com/...`, presented as this project's own process.
- Line 27: "please review our [copying guidelines](COPYING.md)" — fine,
  keep as-is.
- Line 31: security-disclosure link to `modrinth.com/legal/security`,
  presented as "our" process.
- Line 35: "our support page" / "our Discord server" linking to Modrinth's
  actual `support.modrinth.com` and `discord.modrinth.com`.
- Lines 37–39: generic License section — fine to keep, though it should
  probably note this is a fork.

Nothing in the file is phrased as attribution ("built on Modrinth's open
source code"); it's uniformly first-person as-if-official.

`COPYING.md` (21 lines) is Rinth, Inc.'s rights-reservation notice: it
prohibits use of Modrinth's branding, lists the specific prohibited asset
files (see §1), and states forks must remove them. **It has no section for
a fork to declare its own name, license posture, or branding** — it's
purely upstream's notice, with no template or placeholder for a fork's own
statement. Note also: `README.md`'s own line-1 cover image
(`monorepo_cover.png`) is literally one of the assets `COPYING.md` lists as
prohibited — i.e., the README is currently in live violation of this
repo's own copying rules.

### Proposed fix

This section outlines what needs to change, not final copy (per the
original request, wording is a separate pass).

**README.md:**
- Replace or drop the cover image (needs a Sleepless Launcher asset, or no
  image at all).
- Replace or drop the `modrinth/code`-scoped badges; if kept, re-point them
  at this fork's own repo.
- Rewrite the heading and intro paragraph to describe this repo accurately:
  a personal fork of Modrinth's open-source launcher, adding a CurseForge
  content provider — attribution, not identity.
- Replace the "download the app" link with this fork's own releases page.
- Decide, per dev-docs link, whether to keep pointing at upstream docs with
  a "may not reflect this fork's changes" caveat, or write fork-specific
  docs later.
- Replace the security-disclosure and support/Discord links with this
  fork's own equivalents, or state plainly that no formal process exists
  yet if that's the case.

**COPYING.md:**
- Add a new section (or a sibling file, e.g. `COPYING-FORK.md`, linked from
  the main one) stating this fork's own name, that it is an independent
  personal fork not affiliated with or endorsed by Rinth, Inc., and its own
  stance on the assets it has added/replaced. Keep the existing
  Modrinth-branding-prohibition section intact — it's still legally
  load-bearing regardless of this fork's own additions.

### Blocked on repo owner

- Final wording/tone for both files.
- Whether to keep any upstream-docs links or write fork-specific ones.
- Whether a real security-disclosure process/contact exists yet for this
  fork.

---

## 3. Live-backend / official-client branding audit

**Status: done**, with one open item. All flagged "Modrinth App" identity
strings and hardcoded `support.modrinth.com`/`github.com/modrinth/code`
links were rewritten (support/issue links now point at
`github.com/L0stFromSleep/SleeplessLauncher/issues`, per repo owner's choice).
Genuine Modrinth support-article deep links (real Minecraft/Xbox
sign-in troubleshooting docs) were deliberately left as-is — they're
accurate regardless of fork branding. `en-US/index.json` was regenerated
via `pnpm intl:extract`; **other locale files (~15 languages) are still
stale** and need a real translation pass/Crowdin sync, not something
fabricatable here.
>
> **Resolved:** `SurveyPopup.vue` was fully wired to Modrinth's live
> infrastructure — fetching `https://api.modrinth.com/v2/surveys`, opening
> Modrinth's real Tally survey forms, sending the user's real Modrinth
> account ID, with copy claiming "This feedback will go directly to the
> Modrinth team." Per repo owner's decision, the component and its usage in
> `App.vue` were removed entirely (not just the wording) — no user data is
> sent to Modrinth's survey system under Sleepless Launcher branding
> anymore. `en-US/index.json` was re-extracted afterward to drop the
> now-orphaned survey strings.

### Discord Application ID — already done

`packages/app-lib/src/state/discord.rs:16` already uses a Discord
Application ID the repo owner registered themselves
(`1541148253175816242`), not Modrinth's. **No action needed here** — this
item is complete from earlier work, despite being flagged as outstanding in
the original request.

### Frontend copy audit

Searched `apps/app-frontend/src` for strings that assume this app IS the
official Modrinth client, separately from strings that accurately describe
real Modrinth account/API integration (which this fork genuinely has, and
which should stay as-is).

**Needs rewriting — claims to be the official "Modrinth App":**

| File | Line(s) | Issue |
|---|---|---|
| `App.vue` | ~1337, 1341, 1346 | Update-available banner text: `"Modrinth App v{version} is available..."` etc. |
| `apps/app-frontend/index.html` | 7 | `<title>Modrinth App</title>` — browser/window title |
| `ErrorModal.vue` | 79, 167, 181, 220, 234, 250, 260 | Title + body copy referring to "the Modrinth App" |
| `SurveyPopup.vue` | 37, 42 | "Hey there Modrinth user!" / "...your experience with Modrinth App?" |
| `HostingUpdateRequired.vue` | 18, 22 | "Modrinth App update required" — only the app-name part needs changing; "Modrinth Hosting" is a real product name and can stay |
| `settings/account/PrivacySettings.vue` | 52 | "Show Modrinth App as your current activity on Discord" — Rich Presence should show this fork's name |
| `settings/display/BehaviorSettings.vue` | 64 | "Minimize Modrinth App when Minecraft starts." |
| `settings/instances/ResourceManagementSettings.vue` | 37 | "Where Modrinth App stores instances and other files..." |
| `new-icon-editor-notification/apply-new-icons-modal.vue` | 53 | "...right in the Modrinth App..." |
| `pages/instance/components/settings-modal/shared-instance-installation-settings-controls.vue` | 172 | "...People using it in the Modrinth App will stop receiving updates..." (only the app-name part; "Modrinth's servers" elsewhere in the same string is accurate) |

Also: **stale compiled locale JSON** — `apps/app-frontend/src/locales/*/index.json`
mirrors several of the above strings across roughly 15 locale files (e.g.
`de-DE`, `zh-TW`, `fr-FR`, `pt-BR`, `ja-JP`, `pl-PL`, `it-IT`, `zh-CN`,
`hu-HU`, `uk-UA`, `sv-SE`, `ru-RU`). Notably, `en-US/index.json:1317` still
has `"Modrinth App {version}"` even though the source
(`AppSettingsModal.vue:280`) was already correctly updated to `'Sleepless
Launcher {version}'` — the compiled locale files haven't been regenerated
since that source edit landed, so fixing source strings alone is not
sufficient; whatever i18n extraction step produces these JSON files needs
to be re-run afterward.

**Needs rewriting, operationally important (misdirects real users to
Modrinth's real support/issue tracker):**

| File | Line(s) |
|---|---|
| `ErrorModal.vue` | 38, 64, 76, 81, 85, 171, 185 |
| `minecraft-auth-error-modal/MinecraftAuthErrorModal.vue` | 133 |
| `modal/ModrinthAccountRequiredModal.vue` | 213 |
| `minecraft-required-modal/MinecraftRequiredModal.vue` | 24 |
| `shared-instances/shared-instance-install-modal/index.vue` | 37, 44 |
| `minecraft-auth-error-modal/minecraft-auth-errors.ts` | 66 |

These hardcode `support.modrinth.com` and
`github.com/modrinth/code/issues` as the app's own support channel. This is
flagged as higher-urgency than pure branding: a user hitting an actual
Sleepless-Launcher-specific bug is currently directed to file it against
the real Modrinth repository, which is both confusing for the user and
noise for Modrinth's maintainers.

**Accurate — keep as-is, no action:**

- "Sign in to your Modrinth account" and similar copy describing real
  Modrinth account/API integration.
- `AppSettingsModal.vue:280` — already correctly says "Sleepless Launcher
  {version}".
- `WelcomeScreen.vue` — already correctly rebranded, uses
  `sleepless-launcher-logo.png`.
- Ads-consent copy (`App.vue:515`, `PrivacySettings.vue:30`): "Ads make
  Modrinth possible and fund creator payouts." — accurate as long as this
  fork still runs Modrinth's real ad integration and payout program
  unmodified; flag for a decision only if that integration is ever changed
  or stripped.

No hardcoded Discord invite URL was found in `apps/app-frontend/src` — the
only Discord-related string is the Rich Presence activity-name setting
above.

### Proposed fix

A bulk pass over the "needs rewriting" files: replace first-person
"Modrinth App" identity claims with "Sleepless Launcher" (leaving genuine
"Modrinth account"/"Modrinth's servers"/"Modrinth Hosting" references
untouched, since those describe real integration, not identity). Separately,
repoint the hardcoded support/issues URLs at this fork's own channels (new
GitHub repo's issues page, and a support channel if one exists — see
"blocked on" below). After the source edits, re-run whatever command
extracts/regenerates `apps/app-frontend/src/locales/*/index.json` from the
updated `defaultMessage` values, so the ~15 locale files don't drift back
out of sync the way `en-US` already has.

### Action item — not code

Send Modrinth a short heads-up before any wide public release. This fork
authenticates real Modrinth user accounts and rides Modrinth's live API and
social graph, under separate branding. A brief courtesy notice ahead of a
public release meaningfully reduces the chance this reads as unannounced
impersonation, independent of how thoroughly the branding/copy items above
get addressed.

### Blocked on repo owner

- Where this fork's own support/issue-reporting should point (presumably
  this repo's own GitHub Issues, but confirm — and whether a support
  channel like a Discord server exists yet).
- Confirmation that the ads/creator-payout integration is intentionally
  still live in this fork as-is.

---

## 4. CurseForge client rate limiting

**Status: done.** Generalized `util/fetch.rs`'s `is_api_url`-gated
`GLOBAL_API_RATE_LIMIT` into `rate_limit_for_url()`, a lookup by known API
base URL (Modrinth's two, plus CurseForge's), each backed by its own
`ApiRateLimit` instance sharing the same 429/`Retry-After` handling. Added
`CURSEFORGE_API_RATE_LIMIT` at a conservative `Quota::per_minute(60)`
(half Modrinth's local limit), since CurseForge publishes no documented
figure. Verified with `cargo check -p theseus` and the existing
`util::fetch::tests` suite (6/6 passing, unchanged).

### Current state

`packages/app-lib/src/state/curseforge/client.rs`'s own module doc admits
the gap directly:

> "No response caching is implemented here (unlike the Modrinth path in
> `state::cache`), and CurseForge's own documented rate limits are not
> enforced -- both are acceptable for this MVP slice but should be revisited
> before heavier use."

`fetch_curseforge` (the shared helper every CurseForge client function
routes through) only passes through the generic `fetch::fetch_advanced`
with `state.fetch_semaphore` — a plain `tokio::sync::Semaphore`
concurrency cap (`Semaphore::new(settings.max_concurrent_downloads)`),
constructed in `state/mod.rs`. This bounds how many requests can be
*in flight simultaneously*; it has no time dimension and cannot enforce a
requests-per-second/minute limit.

Modrinth's own API requests get real protection that CurseForge's do not:
`packages/app-lib/src/util/fetch.rs` has a `GLOBAL_API_RATE_LIMIT` built on
the `governor` crate (`Quota::per_minute(120).allow_burst(50)`), plus
429/`Retry-After` response handling and a `FetchFence` that blocks a route
after repeated failures in a short window. All of this is gated behind an
`is_api_url` check that only matches `MODRINTH_API_URL` /
`MODRINTH_API_URL_V3` — CurseForge's base URL
(`https://api.curseforge.com/v1/`) never satisfies that check, so none of
this machinery currently applies to CurseForge traffic.

The existing batch-size constants in `client.rs` —
`GET_FILES_MAX_BATCH_SIZE`, `GET_MODS_MAX_BATCH_SIZE`,
`GET_FINGERPRINT_MATCHES_MAX_BATCH_SIZE` (all `500`) — reduce request
*count* for bulk lookups (fewer, larger requests instead of many small
ones) but do nothing to pace request *rate*; a large modpack update could
still fire many sequential batched calls back-to-back with no pacing at
all.

**CurseForge's documented rate limit:** checked `docs.curseforge.com/rest-api/`
directly — it does not publish a rate limit figure at all (confirmed via
direct fetch of the page). A web search for third-party reports (e.g.
`itzg/docker-minecraft-server` GitHub issues) confirms this is a known
community pain point — people do hit undisclosed throttling and 429s, with
anecdotal reports of roughly hour-long cooldowns — but there is no official
published requests-per-minute number to cite. This plan states that
explicitly rather than inventing a figure.

### Proposed fix

Extend the existing `is_api_url` / `GLOBAL_API_RATE_LIMIT` mechanism in
`util/fetch.rs` rather than build a separate CurseForge-specific system:
generalize the single static rate limiter into a small per-API registry
keyed by URL prefix (Modrinth's two API base URLs, plus
`CURSEFORGE_API_BASE` = `https://api.curseforge.com/v1/`), so
`fetch_advanced_with_client_and_progress` looks up which limiter (if any)
applies to a given request instead of hardcoding Modrinth. Since no
official CurseForge RPM figure exists, start with a conservative default
(e.g. `Quota::per_minute(60)`, half Modrinth's) and lean on the
already-implemented 429/`Retry-After` handling and `FetchFence` to adapt if
the real limit turns out stricter — this avoids needing a precise number up
front.

A lighter alternative, if generalizing the shared `fetch.rs` gating logic
feels too invasive for the payoff: add a dedicated
`LazyLock<ApiRateLimit>` static local to `client.rs`, invoked explicitly at
the top of `fetch_curseforge` before it delegates to `fetch_advanced`. Less
code reuse, but a smaller and more isolated change.

### Blocked on repo owner

None — this is fully spec-able as-is; it just needs implementation time.

---

## 5. Git history secret scan

### Plan

Run a full-history secret scan before this repo is ever made public, using
`gitleaks` (recommended over `trufflehog` here: a single static binary with
no Python/Go dependency chain to install, which matters on a Windows dev
machine). Install via `scoop install gitleaks`, or download the appropriate
binary directly from the
[gitleaks GitHub releases page](https://github.com/gitleaks/gitleaks/releases).

Run, from the repo root:

```
gitleaks detect --source . --log-opts="--all"
```

`--log-opts="--all"` ensures every branch/ref is scanned, not just the
current `HEAD` — a secret committed and later removed on `main` is still
exposed in history unless explicitly checked.

**If it finds something:**

1. For any real, live credential: rotate it immediately, regardless of
   whether history gets rewritten. Once a commit containing a secret has
   ever been pushed, treat that value as permanently compromised — a
   force-push or history rewrite after the fact doesn't undo exposure to
   anyone who already had a chance to clone or fork it.
2. Only rewrite history (with `git filter-repo`, not the deprecated
   `git filter-branch`) if the exposed value genuinely can't be rotated, or
   if removing the artifact from clone size/audit trail is wanted for other
   reasons. Treat this as secondary cleanup, not a substitute for rotation.

### Scan results (executed)

Downloaded `gitleaks` v8.30.1 and ran `gitleaks detect --source . --log-opts="--all"` across the full history (4,496 commits, ~81 MB scanned). **18 findings, all pre-existing in history inherited from upstream `modrinth/code` — none introduced by this fork, and none are the repo owner's own credentials:**

- **11 findings — Stripe *publishable* keys** (`pk_live_...`/`pk_test_...`) in `apps/frontend/wrangler.jsonc`, `apps/frontend/src/pages/plus.vue`, `apps/frontend/src/pages/settings/billing/index.vue`, and `.github/workflows/theseus-build.yml`. Publishable keys are meant to be public/client-exposed by Stripe's own design (unlike secret `sk_live_`/`sk_test_` keys) — not a real risk by themselves, and gitleaks' generic-api-key rule doesn't distinguish them. All in `apps/frontend`, the web app, which this fork doesn't build/ship as part of the launcher.
- **2 findings — hCaptcha site key** in `apps/frontend/src/components/ui/HCaptcha.vue`. Site keys are likewise meant to be public (embedded client-side by design, same category as a Google reCAPTCHA site key). Not a real secret.
- **3 findings — local dev default credentials** (`curl -u default:default`, `curl -u elastic:elastic`) in `apps/labrinth/fixtures/analytics-changes-clickhouse.sql` and `docker-compose.yml` — placeholder default credentials for local ClickHouse/Elasticsearch dev containers, not live credentials.
- **1 finding — an example token in an OpenAPI doc comment**, `apps/labrinth/src/routes/mod.rs`: verified by reading the actual commit — it's literally labeled `"Use a personal access token. Example: ..."` and tagged `x-example` for API documentation generation. Not a real token.
- **2 findings — an Nx Cloud access token** in `nx.json` (commits from 2024, authored by original Modrinth contributors, not this fork). The token's suffix decodes to `read-write`, so unlike the others this one is a genuine credential rather than a by-design-public value. It's Modrinth's own Nx Cloud token from their own public open-source history (already exposed in the real, public `modrinth/code` repo, not something introduced or ownable by this fork) — not actionable here since it isn't this fork owner's credential to rotate.

**Conclusion: no action required before making this repo public** — nothing found is a new leak specific to this fork, and the one non-by-design-public credential (Nx Cloud token) belongs to upstream Modrinth's own already-public history, not this fork.

The full JSON report was written to a temp scratch directory during this session and was not committed to the repo (it just restates the above, plus exact byte offsets).
