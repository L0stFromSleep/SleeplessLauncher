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

export interface CfScreenshot {
	title: string | null
	description: string | null
	thumbnailUrl: string | null
	url: string
}

export interface CfMod {
	id: number
	name: string
	summary: string
	logo: CfAsset | null
	links: CfModLinks | null
	classId: number | null
	categories: CfCategory[]
	allowModDistribution: boolean | null
	latestFiles: CfFile[]
	authors: CfAuthor[]
	downloadCount: number | null
	dateCreated: string | null
	dateModified: string | null
	screenshots: CfScreenshot[]
}

/// CurseForge's `classId` values scoping a mod/file to a Minecraft content
/// category, mirroring `packages/app-lib/src/state/curseforge/models.rs`.
export const CF_CLASS_ID = {
	mod: 6,
	modpack: 4471,
	resourcepack: 12,
	datapack: 6945,
	shader: 6552,
} as const

export type CfProjectType = keyof typeof CF_CLASS_ID

export function classIdForProjectType(projectType: string): number | null {
	return CF_CLASS_ID[projectType as CfProjectType] ?? null
}

export function projectTypeForClassId(classId: number | null): CfProjectType | null {
	if (classId === null) return null
	const entry = Object.entries(CF_CLASS_ID).find(([, id]) => id === classId)
	return (entry?.[0] as CfProjectType | undefined) ?? null
}

export interface CurseForgeSearchResults {
	hits: CfMod[]
	total_hits: number
}

/// CurseForge's `SearchSortField` enum. Without passing one, CurseForge
/// returns results in its own default order (roughly "Featured") regardless
/// of what you might expect from an unsorted browse -- there is no implicit
/// "most downloaded"/"most relevant" fallback on their end.
export const CF_SORT_FIELD = {
	popularity: 2,
	lastUpdated: 3,
	totalDownloads: 6,
	releasedDate: 11,
} as const

export async function search(
	query: string,
	gameVersion: string | null,
	classId: number | null,
	sortField: number | null,
	page: number,
	pageSize: number,
): Promise<CurseForgeSearchResults> {
	return await invoke('plugin:curseforge|curseforge_search', {
		query,
		gameVersion,
		classId,
		sortField,
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

export async function getModDescription(modId: string): Promise<string> {
	return await invoke('plugin:curseforge|curseforge_get_mod_description', { modId })
}
