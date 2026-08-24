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
