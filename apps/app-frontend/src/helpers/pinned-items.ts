/**
 * Thin invoke() wrappers for the `pinned-items` Tauri plugin -- the sidebar
 * pin list, covering both client instances and locally hosted servers.
 */
import { invoke } from '@tauri-apps/api/core'

export type PinnedItemKind = 'instance' | 'hosted_server'

export interface PinnedItem {
	kind: PinnedItemKind
	ref_id: string
	position: number
}

export async function list(): Promise<PinnedItem[]> {
	return await invoke('plugin:pinned-items|pinned_items_list')
}

export async function pin(kind: PinnedItemKind, refId: string): Promise<void> {
	return await invoke('plugin:pinned-items|pinned_items_pin', { kind, refId })
}

export async function unpin(kind: PinnedItemKind, refId: string): Promise<void> {
	return await invoke('plugin:pinned-items|pinned_items_unpin', { kind, refId })
}

export async function setOrder(
	ordered: { kind: PinnedItemKind; ref_id: string }[],
): Promise<void> {
	return await invoke('plugin:pinned-items|pinned_items_set_order', { ordered })
}
