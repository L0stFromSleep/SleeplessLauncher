<script setup>
import { PinIcon, SpinnerIcon } from '@modrinth/assets'
import { Avatar, defineMessages, injectNotificationManager, useVIntl } from '@modrinth/ui'
import { useQueryClient } from '@tanstack/vue-query'
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'

import NavButton from '@/components/ui/NavButton.vue'
import { useAppEvent } from '@/composables/use-app-event'
import { usePinnedItems } from '@/composables/use-pinned-items'
import * as hosting from '@/helpers/hosting'
import { getInstanceIconUrl, list as listInstances } from '@/helpers/instance'
import { instanceKeys } from '@/pages/instance/query-options'

const ITEM_SIZE = 52
const APPROX_USED_VERTICAL_SPACE = 475 // doesn't need to be exact lol just close enough so there's a little gap and no overflow
const STORAGE_KEY = 'modrinth-quick-instance-count'

const { handleError } = injectNotificationManager()
const queryClient = useQueryClient()

const { formatMessage } = useVIntl()

const { pinnedItems, refresh: refreshPins } = usePinnedItems()

const maxAuto = ref(0)
const dragging = ref(false)

const instancesById = ref(new Map())
const serversById = ref(new Map())

const stored = localStorage.getItem(STORAGE_KEY)
const userLimit = ref(stored === null ? null : Number(stored))

const pinnedEntries = computed(() =>
	pinnedItems.value
		.map((pin) => {
			if (pin.kind === 'instance') {
				const instance = instancesById.value.get(pin.ref_id)
				if (!instance) return null
				return {
					key: `instance:${instance.id}`,
					to: `/instance/${encodeURIComponent(instance.id)}`,
					name: instance.name,
					iconUrl: getInstanceIconUrl(instance.icon_path),
					installing: instance.install_stage !== 'installed',
				}
			}

			const server = serversById.value.get(pin.ref_id)
			if (!server) return null
			return {
				key: `hosted_server:${server.id}`,
				to: `/host/${encodeURIComponent(server.id)}`,
				name: server.name,
				iconUrl: server.icon_path ?? undefined,
				installing: server.install_stage !== 'installed',
			}
		})
		.filter((entry) => entry !== null),
)

const maxVisible = computed(() => Math.min(maxAuto.value, pinnedEntries.value.length))
const visibleCount = computed(() => Math.min(userLimit.value ?? maxVisible.value, maxVisible.value))
const visibleEntries = computed(() => pinnedEntries.value.slice(0, visibleCount.value))
const canDrag = computed(() => maxVisible.value > 0)
const showOverdrag = ref(false)

const updateMaxAuto = () => {
	maxAuto.value = Math.max(
		0,
		Math.floor((window.innerHeight - APPROX_USED_VERTICAL_SPACE) / ITEM_SIZE),
	)
}

const setLimit = (count) => {
	const clamped = Math.max(0, Math.min(count, maxVisible.value))
	if (clamped >= maxVisible.value) {
		userLimit.value = null
		localStorage.removeItem(STORAGE_KEY)
	} else {
		userLimit.value = clamped
		localStorage.setItem(STORAGE_KEY, String(clamped))
	}
}

let dragStartY = 0
let dragStartCount = 0
let wasOverdragging = false
let overdragTimeout = null

const clearOverdragFlash = () => {
	showOverdrag.value = false
	if (overdragTimeout !== null) {
		clearTimeout(overdragTimeout)
		overdragTimeout = null
	}
}

const flashOverdrag = () => {
	showOverdrag.value = true
	if (overdragTimeout !== null) {
		clearTimeout(overdragTimeout)
	}
	overdragTimeout = setTimeout(() => {
		showOverdrag.value = false
		overdragTimeout = null
	}, 500)
}

const onDividerPointerDown = (event) => {
	if (!canDrag.value) {
		return
	}
	event.preventDefault()
	dragging.value = true
	wasOverdragging = false
	clearOverdragFlash()
	dragStartY = event.clientY
	dragStartCount = visibleCount.value
	document.body.classList.add('quick-instance-dragging')
	event.currentTarget.setPointerCapture(event.pointerId)
}

const onDividerPointerMove = (event) => {
	if (!dragging.value) {
		return
	}
	const delta = event.clientY - dragStartY
	const target = dragStartCount + Math.round(delta / ITEM_SIZE)
	const isOverdragging = target < 0 || target > maxAuto.value
	if (isOverdragging && !wasOverdragging) {
		flashOverdrag()
	}
	wasOverdragging = isOverdragging
	setLimit(target)
}

const endDrag = (event) => {
	if (!dragging.value) {
		return
	}
	dragging.value = false
	wasOverdragging = false
	clearOverdragFlash()
	document.body.classList.remove('quick-instance-dragging')
	if (event?.currentTarget?.hasPointerCapture?.(event.pointerId)) {
		event.currentTarget.releasePointerCapture(event.pointerId)
	}
}

const onDividerPointerUp = (event) => {
	endDrag(event)
}

const getInstances = async () => {
	const instances = await listInstances().catch(handleError)
	if (!instances) return

	for (const instance of instances) {
		queryClient.setQueryData(instanceKeys.detail(instance.id), instance)
	}

	instancesById.value = new Map(instances.map((instance) => [instance.id, instance]))
}

const getServers = async () => {
	try {
		const servers = await hosting.list()
		serversById.value = new Map(servers.map((server) => [server.id, server]))
	} catch (err) {
		handleError(err)
	}
}

const hasPinnedInstances = computed(() =>
	pinnedItems.value.some((pin) => pin.kind === 'instance'),
)
const hasPinnedServers = computed(() =>
	pinnedItems.value.some((pin) => pin.kind === 'hosted_server'),
)

watch(hasPinnedInstances, (has) => {
	if (has) getInstances()
})
watch(hasPinnedServers, (has) => {
	if (has) getServers()
})

defineExpose({ refresh: refreshPins })

if (hasPinnedInstances.value) await getInstances()
if (hasPinnedServers.value) await getServers()
updateMaxAuto()

useAppEvent('instance', async (event) => {
	if (event.event !== 'synced' && hasPinnedInstances.value) {
		await getInstances()
	}
})

onMounted(() => {
	window.addEventListener('resize', updateMaxAuto)
})

onUnmounted(() => {
	window.removeEventListener('resize', updateMaxAuto)
	document.body.classList.remove('quick-instance-dragging')
	clearOverdragFlash()
})

const messages = defineMessages({
	dragTooltip: {
		id: 'app.quick-instance-switcher.drag-tooltip',
		defaultMessage: 'Drag to resize',
	},
	dragShowTooltip: {
		id: 'app.quick-instance-switcher.drag-show-tooltip',
		defaultMessage: 'Drag to show pinned items',
	},
})

const dividerTooltip = computed(() => {
	if (!canDrag.value || dragging.value) {
		return null
	}
	return formatMessage(visibleCount.value === 0 ? messages.dragShowTooltip : messages.dragTooltip)
})
</script>

<template>
	<Transition name="top-divider">
		<div
			v-if="visibleEntries.length > 0"
			class="top-divider flex items-center justify-center overflow-hidden"
		>
			<div class="h-px w-8 bg-surface-5 shrink-0"></div>
		</div>
	</Transition>
	<TransitionGroup name="quick-instance" tag="div" class="flex flex-col items-center">
		<div v-for="entry in visibleEntries" :key="entry.key" v-tooltip.right="entry.name" class="quick-instance-item">
			<NavButton :to="entry.to" class="relative">
				<Avatar
					:src="entry.iconUrl"
					size="28px"
					:tint-by="entry.key"
					:class="`transition-all ${entry.installing ? `brightness-[0.25] scale-[0.85]` : `group-hover:brightness-75`}`"
				/>
				<div
					v-if="entry.installing"
					class="absolute inset-0 flex items-center justify-center z-10 pointer-events-none"
				>
					<SpinnerIcon class="animate-spin w-4 h-4" />
				</div>
				<PinIcon class="absolute -bottom-1 -right-1 z-10 h-3 w-3 rounded-full bg-bg text-secondary" />
			</NavButton>
		</div>
	</TransitionGroup>
	<div
		v-tooltip.right="dividerTooltip"
		class="flex items-center justify-center py-2 select-none"
		:class="canDrag ? 'cursor-ns-resize touch-none group' : ''"
		@pointerdown="onDividerPointerDown"
		@pointermove="onDividerPointerMove"
		@pointerup="onDividerPointerUp"
		@pointercancel="onDividerPointerUp"
	>
		<div
			class="h-px w-8 transition-colors duration-200"
			:class="
				showOverdrag ? 'bg-red' : canDrag ? 'bg-surface-5 group-hover:bg-secondary' : 'bg-surface-5'
			"
		></div>
	</div>
</template>

<style scoped lang="scss">
.top-divider {
	height: calc(1rem + 1px);
}

.top-divider-enter-active,
.top-divider-leave-active {
	transition:
		opacity 0.25s ease,
		height 0.25s ease;
}

.top-divider-enter-from,
.top-divider-leave-to {
	opacity: 0;
	height: 0;
}

.quick-instance-item {
	height: 3rem;
	overflow: hidden;

	& + & {
		margin-top: 0.25rem;
	}
}

.quick-instance-enter-active,
.quick-instance-leave-active {
	transition:
		opacity 0.25s ease,
		transform 0.25s ease,
		height 0.25s ease,
		margin-top 0.25s ease;
}

.quick-instance-enter-from,
.quick-instance-leave-to {
	opacity: 0;
	transform: scale(0.5);
	height: 0;
	margin-top: 0 !important;
}

@media (prefers-reduced-motion: reduce) {
	.top-divider-enter-active,
	.top-divider-leave-active,
	.quick-instance-enter-active,
	.quick-instance-leave-active {
		transition: none;
	}

	.top-divider-enter-from,
	.top-divider-leave-to {
		opacity: 1;
		height: calc(1rem + 1px);
	}

	.quick-instance-enter-from,
	.quick-instance-leave-to {
		opacity: 1;
		transform: none;
		height: 3rem;
		margin-top: unset !important;
	}
}
</style>

<style lang="scss">
body.quick-instance-dragging,
body.quick-instance-dragging * {
	cursor: ns-resize !important;
}
</style>
