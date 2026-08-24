<script setup lang="ts">
import type { Labrinth } from '@modrinth/api-client'
import {
	BoxIcon,
	CompassIcon,
	FolderOpenIcon,
	PinIcon,
	PlusIcon,
	ServerStackIcon,
	SpinnerIcon,
	TrashIcon,
} from '@modrinth/assets'
import {
	Avatar,
	BigOptionButton,
	Button,
	defineMessages,
	IconButton,
	injectNotificationManager,
	StyledInput,
	useVIntl,
} from '@modrinth/ui'
import { open } from '@tauri-apps/plugin-dialog'
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'

import { usePinnedItems } from '@/composables/use-pinned-items'
import type { CreateHostedServerSource, HostedServer } from '@/helpers/hosting'
import * as hosting from '@/helpers/hosting'
import { get_game_versions } from '@/helpers/tags'
import type { InstanceLoader } from '@/helpers/types'
import { useRootBreadcrumb } from '@/providers/breadcrumbs'

const { isPinned, toggle: togglePinnedItem } = usePinnedItems()

const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const router = useRouter()

useRootBreadcrumb({
	slot: 'root',
	id: 'host',
	label: 'Host',
	to: '/host',
	visual: { type: 'icon', component: ServerStackIcon },
})

const messages = defineMessages({
	title: {
		id: 'app.host.title',
		defaultMessage: 'Hosted servers',
	},
	subtitle: {
		id: 'app.host.subtitle',
		defaultMessage:
			'Download a modpack and run it as a dedicated server on this computer.',
	},
	addServer: {
		id: 'app.host.add-server',
		defaultMessage: 'Add server',
	},
	optionCustomTitle: {
		id: 'app.host.option-custom-title',
		defaultMessage: 'Custom setup',
	},
	optionCustomDescription: {
		id: 'app.host.option-custom-description',
		defaultMessage: 'Choose a Minecraft version and loader yourself, no modpack',
	},
	optionSearchTitle: {
		id: 'app.host.option-search-title',
		defaultMessage: 'Choose a modpack',
	},
	optionSearchDescription: {
		id: 'app.host.option-search-description',
		defaultMessage: 'Browse Modrinth and CurseForge modpacks',
	},
	optionImportTitle: {
		id: 'app.host.option-import-title',
		defaultMessage: 'Import a modpack file',
	},
	optionImportDescription: {
		id: 'app.host.option-import-description',
		defaultMessage: 'Use a .mrpack or CurseForge modpack .zip already on your computer',
	},
	nameLabel: {
		id: 'app.host.name-label',
		defaultMessage: 'Server name',
	},
	gameVersionLabel: {
		id: 'app.host.game-version-label',
		defaultMessage: 'Minecraft version',
	},
	loaderLabel: {
		id: 'app.host.loader-label',
		defaultMessage: 'Loader',
	},
	create: {
		id: 'app.host.create',
		defaultMessage: 'Create',
	},
	back: {
		id: 'app.host.back',
		defaultMessage: 'Back',
	},
	empty: {
		id: 'app.host.empty',
		defaultMessage: 'No hosted servers yet. Add one to get started.',
	},
	delete: {
		id: 'app.host.delete',
		defaultMessage: 'Delete',
	},
	pin: {
		id: 'app.host.pin',
		defaultMessage: 'Pin to sidebar',
	},
	unpin: {
		id: 'app.host.unpin',
		defaultMessage: 'Unpin from sidebar',
	},
})

const servers = ref<HostedServer[]>([])
const loading = ref(true)
const creating = ref(false)

type AddMode = 'closed' | 'choose' | 'custom'
const addMode = ref<AddMode>('closed')

async function refresh() {
	loading.value = true
	try {
		servers.value = await hosting.list()
	} catch (err) {
		handleError(err as Error)
	} finally {
		loading.value = false
	}
}

// Polls quietly (no loading spinner) while any server is actively
// installing, so the icon/loader this page shows catches up to what the
// backend resolves partway through install (see hosting/install.rs) instead
// of sitting on stale "vanilla"/blank-icon placeholders until the next
// manual refresh or page mount.
async function pollWhileInstalling() {
	try {
		servers.value = await hosting.list()
	} catch {
		// Best-effort -- a transient poll failure shouldn't disrupt the page.
	}
}

let installPollHandle: ReturnType<typeof setInterval> | null = null
watch(
	() => servers.value.some((server) => server.install_stage === 'installing'),
	(anyInstalling) => {
		if (anyInstalling && !installPollHandle) {
			installPollHandle = setInterval(pollWhileInstalling, 1500)
		} else if (!anyInstalling && installPollHandle) {
			clearInterval(installPollHandle)
			installPollHandle = null
		}
	},
	{ immediate: true },
)

onBeforeUnmount(() => {
	if (installPollHandle) clearInterval(installPollHandle)
})

onMounted(refresh)

function openAddServer() {
	addMode.value = 'choose'
}

function closeAddServer() {
	addMode.value = 'closed'
	newName.value = ''
	newGameVersion.value = ''
	newLoader.value = 'vanilla'
}

async function finishCreate(name: string, source: CreateHostedServerSource) {
	creating.value = true
	try {
		const server = await hosting.create(name, source)
		closeAddServer()
		await refresh()
		await router.push(`/host/${server.id}`)
	} catch (err) {
		handleError(err as Error)
	} finally {
		creating.value = false
	}
}

// -- Custom setup (vanilla + explicit loader/version) --

const gameVersions = ref<Labrinth.Tags.v2.GameVersion[]>([])
const loaderOptions: { id: InstanceLoader; label: string }[] = [
	{ id: 'vanilla', label: 'Vanilla' },
	{ id: 'fabric', label: 'Fabric' },
	{ id: 'forge', label: 'Forge' },
	{ id: 'neoforge', label: 'NeoForge' },
	{ id: 'quilt', label: 'Quilt' },
]
const newName = ref('')
const newGameVersion = ref('')
const newLoader = ref<InstanceLoader>('vanilla')

const releaseGameVersions = computed(() =>
	gameVersions.value.filter((v) => v.version_type === 'release'),
)
const canCreateCustom = computed(() => !!newName.value.trim() && !!newGameVersion.value.trim())

async function openCustomSetup() {
	addMode.value = 'custom'
	if (gameVersions.value.length === 0) {
		try {
			gameVersions.value = await get_game_versions()
		} catch (err) {
			handleError(err as Error)
		}
	}
	if (!newGameVersion.value) {
		newGameVersion.value = releaseGameVersions.value[0]?.version ?? ''
	}
}

async function createCustomServer() {
	await finishCreate(newName.value.trim(), {
		type: 'vanilla',
		game_version: newGameVersion.value,
		loader: newLoader.value === 'vanilla' ? null : newLoader.value,
	})
}

// -- Choose a modpack: reuses the real Browse page (same UI as picking a
// modpack for a client instance) rather than a bespoke search list, via a
// dedicated route (see routes.js's /browse/host/:projectType) rather than a
// query param. Browse.vue detects route.meta.forHost and, for a modpack
// install, creates a hosted server (hosting.create()) and routes to
// /host/:id instead of creating a client instance.
function browseForModpack() {
	router.push('/browse/host/modpack')
}

// -- Import a local modpack file --

async function importModpackFile() {
	const path = await open({
		multiple: false,
		filters: [{ name: 'Modpack', extensions: ['mrpack', 'zip'] }],
	})
	if (!path || typeof path !== 'string') return

	const fileName = path.split(/[\\/]/).pop() ?? 'Imported server'
	const name = fileName.replace(/\.(mrpack|zip)$/i, '')
	await finishCreate(name, { type: 'local_modpack_file', path })
}

async function togglePin(server: HostedServer, event: MouseEvent) {
	event.stopPropagation()
	try {
		await togglePinnedItem('hosted_server', server.id)
	} catch (err) {
		handleError(err as Error)
	}
}

async function deleteServer(server: HostedServer, event: MouseEvent) {
	event.stopPropagation()
	try {
		await hosting.remove(server.id)
		await refresh()
	} catch (err) {
		handleError(err as Error)
	}
}

function openServer(server: HostedServer) {
	router.push(`/host/${server.id}`)
}

function stageLabel(stage: HostedServer['install_stage']): string {
	switch (stage) {
		case 'not_installed':
			return 'Not installed'
		case 'installing':
			return 'Installing…'
		case 'installed':
			return 'Ready'
		case 'error':
			return 'Install failed'
		default:
			return stage
	}
}
</script>

<template>
	<div class="flex flex-col gap-4 p-6">
		<div class="flex items-center justify-between">
			<div>
				<h1 class="m-0 text-2xl font-extrabold text-contrast">
					{{ formatMessage(messages.title) }}
				</h1>
				<p class="m-0 mt-1 text-secondary">{{ formatMessage(messages.subtitle) }}</p>
			</div>
			<Button v-if="addMode === 'closed'" color="brand" @click="openAddServer">
				<PlusIcon />
				{{ formatMessage(messages.addServer) }}
			</Button>
		</div>

		<div
			v-if="addMode === 'choose'"
			class="flex flex-col gap-2 rounded-2xl border border-solid border-surface-4 bg-surface-2 p-4"
		>
			<BigOptionButton
				:icon="BoxIcon"
				:title="formatMessage(messages.optionCustomTitle)"
				:description="formatMessage(messages.optionCustomDescription)"
				@click="openCustomSetup"
			/>
			<BigOptionButton
				:icon="CompassIcon"
				:title="formatMessage(messages.optionSearchTitle)"
				:description="formatMessage(messages.optionSearchDescription)"
				@click="browseForModpack"
			/>
			<BigOptionButton
				:icon="FolderOpenIcon"
				:title="formatMessage(messages.optionImportTitle)"
				:description="formatMessage(messages.optionImportDescription)"
				@click="importModpackFile"
			/>
			<Button type="transparent" @click="closeAddServer">{{ formatMessage(messages.back) }}</Button>
		</div>

		<div
			v-else-if="addMode === 'custom'"
			class="flex flex-col gap-3 rounded-2xl border border-solid border-surface-4 bg-surface-2 p-4"
		>
			<StyledInput v-model="newName" :placeholder="formatMessage(messages.nameLabel)" />
			<label class="flex flex-col gap-1 text-sm text-secondary">
				{{ formatMessage(messages.gameVersionLabel) }}
				<select
					v-model="newGameVersion"
					class="rounded-xl border border-solid border-surface-4 bg-surface-1 p-2 text-secondary"
				>
					<option v-for="v in releaseGameVersions" :key="v.version" :value="v.version">
						{{ v.version }}
					</option>
				</select>
			</label>
			<label class="flex flex-col gap-1 text-sm text-secondary">
				{{ formatMessage(messages.loaderLabel) }}
				<select
					v-model="newLoader"
					class="rounded-xl border border-solid border-surface-4 bg-surface-1 p-2 text-secondary"
				>
					<option v-for="opt in loaderOptions" :key="opt.id" :value="opt.id">{{ opt.label }}</option>
				</select>
			</label>
			<div class="flex justify-between">
				<Button type="transparent" @click="addMode = 'choose'">{{ formatMessage(messages.back) }}</Button>
				<Button color="brand" :disabled="!canCreateCustom || creating" @click="createCustomServer">
					<SpinnerIcon v-if="creating" class="animate-spin" />
					{{ formatMessage(messages.create) }}
				</Button>
			</div>
		</div>

		<div v-if="loading" class="flex justify-center p-8">
			<SpinnerIcon class="size-8 animate-spin text-secondary" />
		</div>
		<div v-else-if="servers.length === 0 && addMode === 'closed'" class="p-8 text-center text-secondary">
			{{ formatMessage(messages.empty) }}
		</div>
		<div v-else class="flex flex-col gap-2">
			<div
				v-for="server in servers"
				:key="server.id"
				class="flex cursor-pointer items-center gap-4 rounded-2xl border border-solid border-surface-4 bg-surface-2 p-4 transition-colors hover:bg-surface-3"
				@click="openServer(server)"
			>
				<Avatar
					:src="hosting.getHostedServerIconUrl(server.icon_path) ?? undefined"
					:alt="server.name"
					size="3rem"
					no-shadow
				/>
				<div class="flex min-w-0 flex-1 flex-col">
					<span class="truncate font-semibold text-contrast">{{ server.name }}</span>
					<span class="text-sm text-secondary">
						{{ server.game_version }} · {{ server.loader }} · {{ stageLabel(server.install_stage) }}
					</span>
				</div>
				<IconButton
					:label="formatMessage(isPinned('hosted_server', server.id) ? messages.unpin : messages.pin)"
					@click="(e: MouseEvent) => togglePin(server, e)"
				>
					<PinIcon
						:style="
							isPinned('hosted_server', server.id)
								? 'color: var(--color-text-default); fill: var(--color-text-default)'
								: undefined
						"
					/>
				</IconButton>
				<IconButton :label="formatMessage(messages.delete)" @click="(e: MouseEvent) => deleteServer(server, e)">
					<TrashIcon />
				</IconButton>
			</div>
		</div>
	</div>
</template>
