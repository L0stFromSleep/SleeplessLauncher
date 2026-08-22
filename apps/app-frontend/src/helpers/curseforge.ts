/**
 * Thin invoke() wrappers for the `curseforge` Tauri plugin, mirroring the
 * style of helpers/cache.js. These types describe the Tauri-command-shaped
 * CurseForge data (which mirrors CurseForge's own REST schema, including its
 * camelCase field names) rather than a Labrinth API response, so they're
 * defined locally instead of in @modrinth/api-client.
 */
import { invoke } from '@tauri-apps/api/core'

export interface CfAsset {
	url: string
}

export interface CfModLinks {
	websiteUrl: string | null
}

export interface CfCategory {
	name: string
}

export interface CfAuthor {
	name: string
}

export interface CfFileHash {
	value: string
	algo: number
}

export interface CfFileDependency {
	modId: number
	relationType: number
}

export interface CfFile {
	id: number
	modId: number
	fileName: string
	displayName: string
	fileDate: string
	downloadUrl: string | null
	hashes: CfFileHash[]
	dependencies: CfFileDependency[]
	gameVersions: string[]
}

export interface CfMod {
	id: number
	name: string
	summary: string
	logo: CfAsset | null
	links: CfModLinks | null
	categories: CfCategory[]
	allowModDistribution: boolean | null
	latestFiles: CfFile[]
	authors: CfAuthor[]
	downloadCount: number | null
	dateCreated: string | null
	dateModified: string | null
}

export interface CurseForgeSearchResults {
	hits: CfMod[]
	total_hits: number
}

export async function search(
	query: string,
	gameVersion: string | null,
	page: number,
	pageSize: number,
): Promise<CurseForgeSearchResults> {
	return await invoke('plugin:curseforge|curseforge_search', {
		query,
		gameVersion,
		page,
		pageSize,
	})
}

export async function getMod(modId: string): Promise<CfMod> {
	return await invoke('plugin:curseforge|curseforge_get_mod', { modId })
}

export async function getModFiles(modId: string): Promise<CfFile[]> {
	return await invoke('plugin:curseforge|curseforge_get_mod_files', { modId })
}
