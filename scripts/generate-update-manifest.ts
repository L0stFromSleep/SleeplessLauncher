/**
 * Builds the `latest.json` manifest that tauri-plugin-updater polls
 * (apps/app/tauri-release.conf.json's `plugins.updater.endpoints`, currently
 * a GitHub Releases "latest" alias) from the signed installer that
 * `tauri build --config tauri-release.conf.json --features updater` just
 * produced. Used by scripts/release-app.sh and release-app.ps1.
 *
 * Usage: node scripts/run.mjs generate-update-manifest v1.2.3 owner/repo
 */

import * as fs from 'fs'
import { dirname, join } from 'path'
import { fileURLToPath } from 'url'

const __dirname = dirname(fileURLToPath(import.meta.url))
const REPO_ROOT = join(__dirname, '..')

function main() {
	const tag = process.argv[2]
	const repoSlug = process.argv[3]
	if (!tag || !repoSlug) {
		console.error('Usage: node scripts/run.mjs generate-update-manifest <tag> <owner/repo>')
		console.error('Example: node scripts/run.mjs generate-update-manifest v1.2.3 L0stFromSleep/SleeplessLauncher')
		process.exit(1)
	}

	const version = tag.replace(/^v/, '')
	const nsisDir = join(REPO_ROOT, 'target/release/bundle/nsis')

	if (!fs.existsSync(nsisDir)) {
		console.error(`${nsisDir} does not exist. Run the release build first.`)
		process.exit(1)
	}

	// target/release/bundle/nsis/ can accumulate installers from previous local
	// builds of other versions (tauri build doesn't clean it between runs), so
	// don't just grab the first *-setup.exe found -- match the version being
	// released, e.g. "Sleepless Launcher_1.2.3_x64-setup.exe".
	const candidates = fs
		.readdirSync(nsisDir)
		.filter((name) => name.endsWith('-setup.exe') && !name.endsWith('.sig'))
	const setupExe =
		candidates.find((name) => name.includes(`_${version}_`)) ?? candidates[0]
	if (!setupExe) {
		console.error(
			`No *-setup.exe found in ${nsisDir}. Did the build run with --config tauri-release.conf.json?`,
		)
		process.exit(1)
	}
	if (!setupExe.includes(`_${version}_`)) {
		console.error(
			`${setupExe} in ${nsisDir} doesn't match version ${version} -- stale build from a ` +
				'previous release? Clean target/release/bundle/nsis/ and rebuild.',
		)
		process.exit(1)
	}

	const sigPath = join(nsisDir, `${setupExe}.sig`)
	if (!fs.existsSync(sigPath)) {
		console.error(
			`Missing signature file ${sigPath}. Was the build run with TAURI_SIGNING_PRIVATE_KEY ` +
				'and TAURI_SIGNING_PRIVATE_KEY_PASSWORD set?',
		)
		process.exit(1)
	}
	const signature = fs.readFileSync(sigPath, 'utf8').trim()

	const manifest = {
		version,
		notes: `See the release notes: https://github.com/${repoSlug}/releases/tag/${tag}`,
		pub_date: new Date().toISOString(),
		platforms: {
			'windows-x86_64': {
				signature,
				url: `https://github.com/${repoSlug}/releases/download/${tag}/${setupExe}`,
			},
		},
	}

	const outPath = join(nsisDir, 'latest.json')
	fs.writeFileSync(outPath, JSON.stringify(manifest, null, '\t') + '\n', 'utf8')
	console.log(`Wrote update manifest to ${outPath}`)
}

main()
