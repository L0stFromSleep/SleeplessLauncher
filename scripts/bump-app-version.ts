/**
 * Sets the desktop app's version (in apps/app-frontend/package.json, which
 * apps/app/tauri.conf.json reads its `version` from, and apps/app/Cargo.toml)
 * from a release tag. Used by scripts/release-app.sh and release-app.ps1.
 *
 * Usage: node scripts/run.mjs bump-app-version v1.2.3
 */

import * as fs from 'fs'
import { dirname, join } from 'path'
import { fileURLToPath } from 'url'

const __dirname = dirname(fileURLToPath(import.meta.url))
const REPO_ROOT = join(__dirname, '..')

function main() {
	const tag = process.argv[2]
	if (!tag) {
		console.error('Usage: node scripts/run.mjs bump-app-version <tag>')
		console.error('Example: node scripts/run.mjs bump-app-version v1.2.3')
		process.exit(1)
	}

	const version = tag.replace(/^v/, '')
	if (!/^\d+\.\d+\.\d+$/.test(version)) {
		console.error(`Tag '${tag}' does not look like a semver version (expected e.g. v1.2.3)`)
		process.exit(1)
	}

	const frontendPkgPath = join(REPO_ROOT, 'apps/app-frontend/package.json')
	const frontendPkg = fs.readFileSync(frontendPkgPath, 'utf8')
	const frontendVersionPattern = /^(\t"version":\s*")[^"]*(")/m
	if (!frontendVersionPattern.test(frontendPkg)) {
		console.error(`Could not find a "version" field to update in ${frontendPkgPath}`)
		process.exit(1)
	}
	const updatedFrontendPkg = frontendPkg.replace(frontendVersionPattern, `$1${version}$2`)
	fs.writeFileSync(frontendPkgPath, updatedFrontendPkg, 'utf8')

	const cargoTomlPath = join(REPO_ROOT, 'apps/app/Cargo.toml')
	const cargoToml = fs.readFileSync(cargoTomlPath, 'utf8')
	const cargoVersionPattern = /^version = "[^"]*"/m
	if (!cargoVersionPattern.test(cargoToml)) {
		console.error(`Could not find a "version" field to update in ${cargoTomlPath}`)
		process.exit(1)
	}
	const updatedCargoToml = cargoToml.replace(cargoVersionPattern, `version = "${version}"`)
	fs.writeFileSync(cargoTomlPath, updatedCargoToml, 'utf8')

	console.log(`Bumped app version to ${version}`)
}

main()
