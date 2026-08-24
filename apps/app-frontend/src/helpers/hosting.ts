/**
 * Thin invoke() wrappers for the `hosting` Tauri plugin -- locally
 * self-hosted Minecraft servers, mirroring the style of helpers/curseforge.ts.
 */
import { convertFileSrc, invoke } from '@tauri-apps/api/core'

import type { InstanceLoader } from './types'

/** Resolves a hosted server's stored `icon_path` (a raw filesystem path, or
 *  a remote URL for the rare case one is stored directly) into something an
 *  `<img>`/`Avatar` can actually load -- mirrors `getInstanceIconUrl`. */
export function getHostedServerIconUrl(iconPath: string | null | undefined): string | null {
	if (!iconPath) return null
	if (iconPath.startsWith('http://') || iconPath.startsWith('https://')) return iconPath
	return convertFileSrc(iconPath)
}

export type HostedServerProvider = 'vanilla' | 'modrinth' | 'curseforge'
export type HostedServerInstallStage = 'not_installed' | 'installing' | 'installed' | 'error'

export interface HostedServer {
	id: string
	name: string
	icon_path: string | null
	game_version: string
	loader: InstanceLoader
	loader_version: string | null
	provider: HostedServerProvider
	project_id: string | null
	version_id: string | null
	port: number
	max_memory_mb: number
	extra_java_args: string | null
	eula_accepted: boolean
	install_stage: HostedServerInstallStage
	created: string
	modified: string
}

export type CreateHostedServerSource =
	| { type: 'vanilla'; game_version: string; loader?: InstanceLoader | null }
	| { type: 'modrinth_modpack'; project_id: string; version_id: string }
	| { type: 'curseforge_modpack'; mod_id: string; file_id: string }
	| { type: 'local_modpack_file'; path: string }

export interface HostedServerProcessMetadata {
	uuid: string
	server_id: string
	start_time: string
}

export async function list(): Promise<HostedServer[]> {
	return await invoke('plugin:hosting|hosting_list')
}

export async function get(id: string): Promise<HostedServer | null> {
	return await invoke('plugin:hosting|hosting_get', { id })
}

export async function create(
	name: string,
	source: CreateHostedServerSource,
): Promise<HostedServer> {
	return await invoke('plugin:hosting|hosting_create', { name, source })
}

export async function remove(id: string): Promise<void> {
	return await invoke('plugin:hosting|hosting_delete', { id })
}

export async function start(id: string): Promise<void> {
	return await invoke('plugin:hosting|hosting_start', { id })
}

export async function stop(id: string): Promise<void> {
	return await invoke('plugin:hosting|hosting_stop', { id })
}

export async function sendCommand(id: string, line: string): Promise<void> {
	return await invoke('plugin:hosting|hosting_send_command', { id, line })
}

export async function isRunning(id: string): Promise<boolean> {
	return await invoke('plugin:hosting|hosting_is_running', { id })
}

export async function getProcess(id: string): Promise<HostedServerProcessMetadata | null> {
	return await invoke('plugin:hosting|hosting_get_process', { id })
}

export async function getLogBuffer(id: string): Promise<string[]> {
	return await invoke('plugin:hosting|hosting_get_log_buffer', { id })
}

export async function setEulaAccepted(id: string, accepted: boolean): Promise<void> {
	return await invoke('plugin:hosting|hosting_set_eula_accepted', { id, accepted })
}

export async function getDirectory(id: string): Promise<string> {
	return await invoke('plugin:hosting|hosting_get_directory', { id })
}

export interface HostedServerContentSummary {
	mods: number
	resourcepacks: number
	datapacks: number
	shaderpacks: number
}

export async function getContentSummary(id: string): Promise<HostedServerContentSummary> {
	return await invoke('plugin:hosting|hosting_content_summary', { id })
}

export type HostedContentProvider = 'modrinth' | 'curseforge'

/** Real project metadata for one installed content file, resolved by hash --
 *  see `state::hosting::content_metadata` on the backend for why this can't
 *  just be read back from install-time records the way a client instance's
 *  content page does. */
export interface HostedContentMetadata {
	provider: HostedContentProvider
	project_id: string
	version_id: string | null
	title: string
	icon_url: string | null
	project_url: string
}

/** Keyed by relative path (e.g. `"mods/somejar.jar"`). Files with no match
 *  on either provider are simply absent from the map. */
export async function getContentMetadata(
	id: string,
): Promise<Record<string, HostedContentMetadata>> {
	return await invoke('plugin:hosting|hosting_content_metadata', { id })
}

export async function setIconPath(id: string, iconPath: string | null): Promise<void> {
	return await invoke('plugin:hosting|hosting_set_icon_path', { id, iconPath })
}

export async function setIconFromPath(id: string, path: string): Promise<void> {
	return await invoke('plugin:hosting|hosting_set_icon_from_path', { id, path })
}

export async function installModrinthFile(
	id: string,
	contentDir: string,
	fileName: string,
	url: string,
	sha1: string | null,
): Promise<void> {
	return await invoke('plugin:hosting|hosting_install_modrinth_file', {
		id,
		contentDir,
		fileName,
		url,
		sha1,
	})
}

export async function installCurseForgeFile(
	id: string,
	contentDir: string,
	fileId: string,
): Promise<void> {
	return await invoke('plugin:hosting|hosting_install_curseforge_file', {
		id,
		contentDir,
		fileId,
	})
}

export async function installLocalContentFile(id: string, path: string): Promise<void> {
	return await invoke('plugin:hosting|hosting_install_local_content_file', { id, path })
}

export async function updateSettings(
	id: string,
	port: number,
	maxMemoryMb: number,
	extraJavaArgs: string | null,
): Promise<void> {
	return await invoke('plugin:hosting|hosting_update_settings', {
		id,
		port,
		maxMemoryMb,
		extraJavaArgs,
	})
}

export type HostedServerDifficulty = 'peaceful' | 'easy' | 'normal' | 'hard'
export type HostedServerGamemode = 'survival' | 'creative' | 'adventure' | 'spectator'

/** The common `server.properties` fields a server admin actually needs day
 *  to day -- not a full raw-properties editor. Everything else in the file
 *  (comments, unmanaged keys like `level-seed`) is left untouched. */
export interface HostedServerProperties {
	motd: string
	max_players: number
	difficulty: HostedServerDifficulty
	gamemode: HostedServerGamemode
	hardcore: boolean
	pvp: boolean
	online_mode: boolean
	white_list: boolean
	enable_command_block: boolean
	allow_flight: boolean
	allow_nether: boolean
	spawn_protection: number
	view_distance: number
	simulation_distance: number
}

export async function getProperties(id: string): Promise<HostedServerProperties> {
	return await invoke('plugin:hosting|hosting_get_properties', { id })
}

export async function setProperties(
	id: string,
	properties: HostedServerProperties,
): Promise<void> {
	return await invoke('plugin:hosting|hosting_set_properties', { id, properties })
}
