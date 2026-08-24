/**
 * Module-level singleton reactive store for the sidebar pin list. Since
 * pinning/unpinning can happen from many different pages (instance library
 * cards, the Host list, detail-page overflow menus) but needs to be
 * reflected instantly in the always-mounted sidebar (QuickInstanceSwitcher),
 * a shared reactive ref is simpler than plumbing a new backend AppEvent
 * end-to-end (hosted servers don't have an event system at all yet) --
 * everything runs in the same webview, so a plain shared ref is enough.
 */
import { ref } from 'vue'

import type { PinnedItem, PinnedItemKind } from '@/helpers/pinned-items'
import * as pinnedItemsApi from '@/helpers/pinned-items'

const pinnedItems = ref<PinnedItem[]>([])
const loaded = ref(false)

async function refresh() {
	pinnedItems.value = await pinnedItemsApi.list()
	loaded.value = true
}

function isPinned(kind: PinnedItemKind, refId: string): boolean {
	return pinnedItems.value.some((item) => item.kind === kind && item.ref_id === refId)
}

async function pin(kind: PinnedItemKind, refId: string) {
	if (isPinned(kind, refId)) return
	await pinnedItemsApi.pin(kind, refId)
	await refresh()
}

async function unpin(kind: PinnedItemKind, refId: string) {
	if (!isPinned(kind, refId)) return
	await pinnedItemsApi.unpin(kind, refId)
	await refresh()
}

async function toggle(kind: PinnedItemKind, refId: string) {
	if (isPinned(kind, refId)) {
		await unpin(kind, refId)
	} else {
		await pin(kind, refId)
	}
}

export function usePinnedItems() {
	if (!loaded.value) {
		refresh()
	}

	return { pinnedItems, refresh, pin, unpin, toggle, isPinned }
}
