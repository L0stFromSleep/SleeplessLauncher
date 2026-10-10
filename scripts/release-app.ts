/**
 * Builds a signed, auto-update-capable Windows build of the desktop app and
 * publishes it (installer + update manifest) as a GitHub release.
 *
 * Requires:
 *   - GitHub CLI (gh), authenticated (`gh auth login`)
 *   - TAURI_SIGNING_PRIVATE_KEY and TAURI_SIGNING_PRIVATE_KEY_PASSWORD set in
 *     the environment (generate a keypair once with:
 *     `pnpm --filter @modrinth/app tauri signer generate -w <path-to-key-file>`,
 *     store the key and password somewhere safe outside the repo, and put the
 *     printed public key into apps/app/tauri-release.conf.json's
 *     plugins.updater.pubkey)
 *
 * Usage: node scripts/run.mjs release-app v1.2.3
 */

import { spawnSync } from 'child_process'
import * as fs from 'fs'
import { dirname, join } from 'path'
import { fileURLToPath } from 'url'

const __dirname = dirname(fileURLToPath(import.meta.url))
const REPO_ROOT = join(__dirname, '..')

function run(command: string, args: string[]) {
	const result = spawnSync(command, args, {
		cwd: REPO_ROOT,
		stdio: 'inherit',
		shell: process.platform === 'win32',
	})
	if (result.error) {
		throw result.error
	}
	if (result.status !== 0) {
		process.exit(result.status ?? 1)
	}
}

function captureOutput(command: string, args: string[]): string {
	const result = spawnSync(command, args, {
		cwd: REPO_ROOT,
		encoding: 'utf8',
		shell: process.platform === 'win32',
	})
	if (result.error) {
		throw result.error
	}
	if (result.status !== 0) {
		console.error(result.stderr)
		process.exit(result.status ?? 1)
	}
	return result.stdout.trim()
}

function commandExists(command: string): boolean {
	const result = spawnSync(command, ['--version'], {
		cwd: REPO_ROOT,
		stdio: 'ignore',
		shell: process.platform === 'win32',
	})
	return !result.error && result.status === 0
}

function main() {
	const tag = process.argv[2]
	if (!tag) {
		console.error('Usage: node scripts/run.mjs release-app <tag>')
		console.error('Example: node scripts/run.mjs release-app v1.2.3')
		process.exit(1)
	}

	if (!commandExists('gh')) {
		console.error(
			"GitHub CLI (gh) is required but was not found on PATH. Install it from https://cli.github.com/ and run 'gh auth login' first.",
		)
		process.exit(1)
	}

	if (!process.env.TAURI_SIGNING_PRIVATE_KEY || !process.env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD) {
		console.error(
			'TAURI_SIGNING_PRIVATE_KEY and TAURI_SIGNING_PRIVATE_KEY_PASSWORD must be set so the update ' +
				"artifacts can be signed (without this, existing installs can't verify and install updates).",
		)
		process.exit(1)
	}

	console.log(`Bumping app version to ${tag}...`)
	run(process.execPath, [
		'--disable-warning=MODULE_TYPELESS_PACKAGE_JSON',
		join(__dirname, 'bump-app-version.ts'),
		tag,
	])

	// tauri build doesn't clean target/release/bundle/ between runs, so a stale
	// installer from a previous local build can otherwise get uploaded alongside
	// (or instead of) this release's, or get picked up by generate-update-manifest.
	console.log('Cleaning previous local build output...')
	fs.rmSync(join(REPO_ROOT, 'target/release/bundle/msi'), { recursive: true, force: true })
	fs.rmSync(join(REPO_ROOT, 'target/release/bundle/nsis'), { recursive: true, force: true })

	console.log('Building app (release, updater-signed)...')
	run('pnpm', [
		'--filter',
		'@modrinth/app',
		'tauri',
		'build',
		'--config',
		'tauri-release.conf.json',
		'--features',
		'updater',
	])

	const msiDir = join(REPO_ROOT, 'target/release/bundle/msi')
	const nsisDir = join(REPO_ROOT, 'target/release/bundle/nsis')
	const assets = [
		...(fs.existsSync(msiDir)
			? fs.readdirSync(msiDir).filter((name) => name.endsWith('.msi'))
					.map((name) => join(msiDir, name))
			: []),
		...(fs.existsSync(nsisDir)
			? fs.readdirSync(nsisDir).filter((name) => name.endsWith('-setup.exe'))
					.map((name) => join(nsisDir, name))
			: []),
	]

	if (assets.length === 0) {
		console.error(
			'No installer files found under target/release/bundle/{msi,nsis}/. Did the build succeed?',
		)
		process.exit(1)
	}

	const repoSlug = captureOutput('gh', ['repo', 'view', '--json', 'nameWithOwner', '-q', '.nameWithOwner'])

	console.log(`Generating update manifest for ${repoSlug}...`)
	run(process.execPath, [
		'--disable-warning=MODULE_TYPELESS_PACKAGE_JSON',
		join(__dirname, 'generate-update-manifest.ts'),
		tag,
		repoSlug,
	])

	console.log(`Creating release '${tag}' with ${assets.length} installer(s) + update manifest...`)
	run('gh', [
		'release',
		'create',
		tag,
		...assets,
		join(nsisDir, 'latest.json'),
		'--title',
		tag,
		'--generate-notes',
	])

	console.log(`Release '${tag}' published. Existing installs will pick up this update automatically.`)
}

main()
