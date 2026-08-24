<script setup lang="ts">
import {
	BoxIcon,
	CompassIcon,
	FolderOpenIcon,
	MoreVerticalIcon,
	PlayIcon,
	SettingsIcon,
	SpinnerIcon,
	StopCircleIcon,
	TerminalSquareIcon,
	TrashIcon,
} from '@modrinth/assets'
import type {
	ContentCardTableItem,
	EditingFile,
	FileItem,
	OverflowMenuOption,
	UploadState,
} from '@modrinth/ui'
import {
	Admonition,
	Avatar,
	Button,
	commonMessages,
	ContentCardTable,
	defineMessages,
	EmptyState,
	FilePageLayout,
	IconButton,
	injectNotificationManager,
	NavTabs,
	NewModal,
	PageHeader,
	PageHeaderActions,
	PageHeaderMetadata,
	PageHeaderMetadataItem,
	provideFileManager,
	ReadyTransition,
	StyledInput,
	TeleportOverflowMenu,
	useVIntl,
} from '@modrinth/ui'
import { open, save } from '@tauri-apps/plugin-dialog'
import {
	mkdir,
	readDir,
	readFile as readFileBytes,
	readTextFile,
	remove,
	rename,
	stat,
	writeFile as writeFileBytes,
	writeTextFile,
} from '@tauri-apps/plugin-fs'
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import * as hosting from '@/helpers/hosting'
import type { HostedServer, HostedServerContentSummary } from '@/helpers/hosting'
import { highlightInFolder } from '@/helpers/utils'
import { useRootBreadcrumb } from '@/providers/breadcrumbs'

const { formatMessage } = useVIntl()
const { handleError, addNotification } = injectNotificationManager()
const route = useRoute()
const router = useRouter()

const serverId = computed(() => route.params.id as string)
const server = ref<HostedServer | null>(null)
const running = ref(false)
const logs = ref<string[]>([])
const commandInput = ref('')
const eulaAccepted = ref(false)
const busy = ref(false)
const contentSummary = ref<HostedServerContentSummary | null>(null)

const portInput = ref(25565)
const memoryInput = ref(4096)
const extraArgsInput = ref('')

const messages = defineMessages({
	start: { id: 'app.host.detail.start', defaultMessage: 'Start' },
	stop: { id: 'app.host.detail.stop', defaultMessage: 'Stop' },
	running: { id: 'app.host.detail.running', defaultMessage: 'Running' },
	stopped: { id: 'app.host.detail.stopped', defaultMessage: 'Stopped' },
	settings: { id: 'app.host.detail.settings-action', defaultMessage: 'Settings' },
	deleteServer: { id: 'app.host.detail.delete-server', defaultMessage: 'Delete server' },
	tabContent: { id: 'app.host.detail.tab-content', defaultMessage: 'Content' },
	tabFiles: { id: 'app.host.detail.tab-files', defaultMessage: 'Files' },
	tabConsole: { id: 'app.host.detail.tab-console', defaultMessage: 'Console' },
	settingsModalTitle: {
		id: 'app.host.detail.settings-modal-title',
		defaultMessage: 'Server settings',
	},
	eulaNotice: {
		id: 'app.host.detail.eula-notice',
		defaultMessage:
			'Starting this server requires accepting the Minecraft End User License Agreement (EULA) on its behalf: https://www.minecraft.net/eula',
	},
	acceptEula: {
		id: 'app.host.detail.accept-eula',
		defaultMessage: 'I accept the Minecraft EULA',
	},
	sendCommand: {
		id: 'app.host.detail.send-command',
		defaultMessage: 'Send',
	},
	commandPlaceholder: {
		id: 'app.host.detail.command-placeholder',
		defaultMessage: 'Type a server command (e.g. /op Notch)…',
	},
	noContent: { id: 'app.host.detail.no-content', defaultMessage: 'No content installed' },
	browseContent: { id: 'app.host.detail.browse-content', defaultMessage: 'Browse content' },
	uploadFiles: { id: 'app.host.detail.upload-files', defaultMessage: 'Upload files' },
	portLabel: { id: 'app.host.detail.port-label', defaultMessage: 'Port' },
	memoryLabel: { id: 'app.host.detail.memory-label', defaultMessage: 'Max memory (MB)' },
	extraArgsLabel: {
		id: 'app.host.detail.extra-args-label',
		defaultMessage: 'Extra JVM arguments',
	},
	saveSettings: { id: 'app.host.detail.save-settings', defaultMessage: 'Save settings' },
	changeIcon: { id: 'app.host.detail.change-icon', defaultMessage: 'Change icon' },
	removeIcon: { id: 'app.host.detail.remove-icon', defaultMessage: 'Remove icon' },
	saveAs: { id: 'app.host.detail.save-as', defaultMessage: 'Save as…' },
})

type TabKey = 'content' | 'files' | 'console'
const activeTab = ref<TabKey>('content')
const tabOrder: TabKey[] = ['content', 'files', 'console']
const tabs = computed(() => [
	{ label: formatMessage(messages.tabContent), href: 'content', icon: BoxIcon },
	{ label: formatMessage(messages.tabFiles), href: 'files', icon: FolderOpenIcon },
	{ label: formatMessage(messages.tabConsole), href: 'console', icon: TerminalSquareIcon },
])
const activeTabIndex = computed(() => tabOrder.indexOf(activeTab.value))

const settingsModal = ref<InstanceType<typeof NewModal> | null>(null)

let pollHandle: ReturnType<typeof setInterval> | null = null
const consoleEl = ref<HTMLElement | null>(null)

async function refreshServer() {
	server.value = await hosting.get(serverId.value)
	if (server.value) {
		eulaAccepted.value = server.value.eula_accepted
		portInput.value = server.value.port
		memoryInput.value = server.value.max_memory_mb
		extraArgsInput.value = server.value.extra_java_args ?? ''
	}
	try {
		contentSummary.value = await hosting.getContentSummary(serverId.value)
	} catch {
		contentSummary.value = null
	}
}

async function poll() {
	try {
		running.value = await hosting.isRunning(serverId.value)
		logs.value = await hosting.getLogBuffer(serverId.value)
		if (activeTab.value === 'console') {
			await nextTick()
			if (consoleEl.value) {
				consoleEl.value.scrollTop = consoleEl.value.scrollHeight
			}
		}
	} catch {
		// Best-effort -- a transient poll failure shouldn't disrupt the page.
	}
}

onMounted(async () => {
	await refreshServer()
	await poll()
	pollHandle = setInterval(poll, 1000)
})

onBeforeUnmount(() => {
	if (pollHandle) clearInterval(pollHandle)
})

watch(serverId, async () => {
	await refreshServer()
	await poll()
})

async function toggleEula(value: boolean) {
	eulaAccepted.value = value
	try {
		await hosting.setEulaAccepted(serverId.value, value)
	} catch (err) {
		handleError(err as Error)
	}
}

async function handleStart() {
	busy.value = true
	try {
		await hosting.start(serverId.value)
		await poll()
	} catch (err) {
		handleError(err as Error)
	} finally {
		busy.value = false
	}
}

async function handleStop() {
	busy.value = true
	try {
		await hosting.stop(serverId.value)
		await poll()
	} catch (err) {
		handleError(err as Error)
	} finally {
		busy.value = false
	}
}

async function sendCommand() {
	if (!commandInput.value.trim()) return
	try {
		await hosting.sendCommand(serverId.value, commandInput.value.trim())
		commandInput.value = ''
	} catch (err) {
		handleError(err as Error)
	}
}

async function saveSettings() {
	try {
		await hosting.updateSettings(
			serverId.value,
			portInput.value,
			memoryInput.value,
			extraArgsInput.value.trim() || null,
		)
		await refreshServer()
		settingsModal.value?.hide()
	} catch (err) {
		handleError(err as Error)
	}
}

const changingIcon = ref(false)

async function changeIcon() {
	const path = await open({
		multiple: false,
		filters: [{ name: 'Image', extensions: ['png', 'jpg', 'jpeg', 'webp', 'gif'] }],
	})
	if (!path || typeof path !== 'string') return

	changingIcon.value = true
	try {
		await hosting.setIconFromPath(serverId.value, path)
		await refreshServer()
	} catch (err) {
		handleError(err as Error)
	} finally {
		changingIcon.value = false
	}
}

async function removeIcon() {
	changingIcon.value = true
	try {
		await hosting.setIconPath(serverId.value, null)
		await refreshServer()
	} catch (err) {
		handleError(err as Error)
	} finally {
		changingIcon.value = false
	}
}

async function deleteServer() {
	try {
		await hosting.remove(serverId.value)
		await router.push('/host')
	} catch (err) {
		handleError(err as Error)
	}
}

const overflowOptions = computed<OverflowMenuOption[]>(() => [
	{
		id: 'delete',
		type: 'action',
		label: formatMessage(messages.deleteServer),
		icon: TrashIcon,
		tone: 'red',
		action: deleteServer,
	},
])

// -- Content tab: a plain file list (mods/resourcepacks/datapacks/
// shaderpacks), not the modpack-summary "View content" card -- hosted
// servers don't participate in the client instance content-set/provider
// tracking system (see hosting/install.rs's module doc), so there's no real
// per-item Modrinth/CurseForge metadata to show; every item is a bare file.

const CONTENT_DIRS: { dir: string; projectType: string }[] = [
	{ dir: 'mods', projectType: 'mod' },
	{ dir: 'resourcepacks', projectType: 'resourcepack' },
	{ dir: 'datapacks', projectType: 'datapack' },
	{ dir: 'shaderpacks', projectType: 'shader' },
]

const contentItems = ref<ContentCardTableItem[]>([])
const contentItemsLoading = ref(true)

async function refreshContentItems() {
	if (!serverRoot.value) return
	contentItemsLoading.value = true
	try {
		const items: ContentCardTableItem[] = []
		for (const { dir } of CONTENT_DIRS) {
			let entries
			try {
				entries = await readDir(resolvePath(dir))
			} catch {
				continue
			}
			for (const entry of entries) {
				if (entry.isDirectory) continue
				items.push({
					id: `${dir}/${entry.name}`,
					project: {
						id: entry.name,
						slug: null,
						title: entry.name,
						icon_url: null,
					},
					version: {
						id: entry.name,
						version_number: '',
						file_name: entry.name,
					},
					external: true,
					hideSwitchVersion: true,
				})
			}
		}
		contentItems.value = items
	} finally {
		contentItemsLoading.value = false
	}
}

async function handleDeleteContentItem(id: string) {
	try {
		await remove(resolvePath(id))
		contentItems.value = contentItems.value.filter((item) => item.id !== id)
		contentSummary.value = await hosting.getContentSummary(serverId.value)
	} catch (err) {
		handleError(err as Error)
	}
}

// "Browse content" pushes to the shared Browse page with a `hs` (hosted
// server id) query param -- Browse.vue installs single mods/resourcepacks/
// datapacks/shaderpacks directly into this server (via hosting.installXFile)
// instead of into a client instance when it sees that param, mirroring how
// it already installs into a client instance when given `i`.
function browseContent() {
	if (!server.value) return
	router.push({
		path: `/browse/${server.value.loader === 'vanilla' ? 'resourcepack' : 'mod'}`,
		query: { hs: serverId.value },
	})
}

const uploadingContent = ref(false)

async function uploadContentFiles() {
	const files = await open({ multiple: true })
	if (!files) return
	const paths = (Array.isArray(files) ? files : [files]).map((file) =>
		typeof file === 'string' ? file : (file as { path: string }).path,
	)

	uploadingContent.value = true
	try {
		for (const path of paths) {
			try {
				await hosting.installLocalContentFile(serverId.value, path)
			} catch (err) {
				handleError(err as Error)
			}
		}
		await refreshContentItems()
		contentSummary.value = await hosting.getContentSummary(serverId.value)
	} finally {
		uploadingContent.value = false
	}
}

// -- Files tab (reuses the same generic FilePageLayout the instance page uses) --

const serverRoot = ref('')
const fileItems = ref<FileItem[]>([])
const filesFirstPaintPending = ref(true)
const filesLoading = ref(true)
const filesError = ref<Error | null>(null)
const currentPath = ref('')
const editingFile = ref<EditingFile | null>(null)

function resolvePath(relativePath: string): string {
	return relativePath ? `${serverRoot.value}/${relativePath}` : serverRoot.value
}

async function listDirectory(dirPath: string): Promise<FileItem[]> {
	const absPath = resolvePath(dirPath)
	const entries = await readDir(absPath)
	const results = await Promise.all(
		entries.map(async (entry) => {
			const entryAbsPath = `${absPath}/${entry.name}`
			let metadata
			try {
				metadata = await stat(entryAbsPath)
			} catch {
				return null
			}
			const item: FileItem = {
				name: entry.name,
				type: entry.isDirectory ? 'directory' : 'file',
				path: dirPath ? `${dirPath}/${entry.name}` : entry.name,
				modified: metadata.mtime ? Math.floor(metadata.mtime.getTime() / 1000) : 0,
				created: metadata.birthtime ? Math.floor(metadata.birthtime.getTime() / 1000) : 0,
			}
			if (!entry.isDirectory) {
				item.size = metadata.size
			}
			if (entry.isDirectory) {
				try {
					const children = await readDir(entryAbsPath)
					item.count = children.length
				} catch {
					item.count = 0
				}
			}
			return item
		}),
	)
	return results.filter((item): item is FileItem => item !== null)
}

async function refreshFiles() {
	filesLoading.value = true
	try {
		fileItems.value = await listDirectory(currentPath.value)
		filesError.value = null
	} catch (err) {
		filesError.value = err as Error
		fileItems.value = []
	} finally {
		filesLoading.value = false
		filesFirstPaintPending.value = false
	}
}

function navigateToPath(path: string) {
	currentPath.value = path.startsWith('/') ? path.slice(1) : path
	refreshFiles()
}

async function handleCreateItem(name: string, type: 'file' | 'directory') {
	const targetPath = currentPath.value ? `${currentPath.value}/${name}` : name
	const absPath = resolvePath(targetPath)
	try {
		if (type === 'directory') {
			await mkdir(absPath)
		} else {
			await writeTextFile(absPath, '')
		}
		await refreshFiles()
	} catch (e) {
		addNotification({
			title: formatMessage(commonMessages.createFailedLabel),
			text: e instanceof Error ? e.message : '',
			type: 'error',
		})
	}
}

async function handleRenameItem(path: string, newName: string) {
	const oldAbs = resolvePath(path)
	const parentDir = path.includes('/') ? path.substring(0, path.lastIndexOf('/')) : ''
	const newAbs = resolvePath(parentDir ? `${parentDir}/${newName}` : newName)
	try {
		await rename(oldAbs, newAbs)
		await refreshFiles()
	} catch (e) {
		addNotification({
			title: formatMessage(commonMessages.renameFailedLabel),
			text: e instanceof Error ? e.message : '',
			type: 'error',
		})
	}
}

async function handleMoveItem(source: string, destination: string) {
	try {
		await rename(resolvePath(source), resolvePath(destination))
		await refreshFiles()
	} catch (e) {
		addNotification({
			title: formatMessage(commonMessages.moveFailedLabel),
			text: e instanceof Error ? e.message : '',
			type: 'error',
		})
	}
}

async function handleDeleteItem(path: string, recursive: boolean) {
	try {
		await remove(resolvePath(path), { recursive })
		await refreshFiles()
	} catch (e) {
		addNotification({
			title: formatMessage(commonMessages.deleteFailedLabel),
			text: e instanceof Error ? e.message : '',
			type: 'error',
		})
	}
}

async function handleReadFile(path: string): Promise<string> {
	return await readTextFile(resolvePath(path))
}

async function handleReadFileAsBlob(path: string): Promise<Blob> {
	const bytes = await readFileBytes(resolvePath(path))
	return new Blob([bytes])
}

async function handleWriteFile(path: string, content: string) {
	await writeTextFile(resolvePath(path), content)
}

async function handleDownloadFile(path: string, fileName: string) {
	const bytes = await readFileBytes(resolvePath(path))
	const dest = await save({ defaultPath: fileName })
	if (!dest) return
	await writeFileBytes(dest, bytes)
}

const uploadState = ref<UploadState>({
	isUploading: false,
	currentFileName: null,
	currentFileProgress: 0,
	uploadedBytes: 0,
	totalBytes: 0,
	completedFiles: 0,
	totalFiles: 0,
})

async function handleUploadFiles(files: File[]) {
	if (files.length === 0) return
	uploadState.value = {
		isUploading: true,
		currentFileName: '',
		currentFileProgress: 0,
		uploadedBytes: 0,
		totalBytes: files.reduce((sum, f) => sum + f.size, 0),
		completedFiles: 0,
		totalFiles: files.length,
	}
	try {
		for (const file of files) {
			uploadState.value.currentFileName = file.name
			const buffer = await file.arrayBuffer()
			const targetPath = resolvePath(
				currentPath.value ? `${currentPath.value}/${file.name}` : file.name,
			)
			await writeFileBytes(targetPath, new Uint8Array(buffer))
			uploadState.value.completedFiles++
			uploadState.value.uploadedBytes += file.size
			uploadState.value.currentFileProgress = 1
		}
	} catch (e) {
		addNotification({
			title: formatMessage(commonMessages.uploadFailedLabel),
			text: e instanceof Error ? e.message : '',
			type: 'error',
		})
	} finally {
		uploadState.value.isUploading = false
		await refreshFiles()
	}
}

provideFileManager({
	items: fileItems,
	loading: filesLoading,
	error: filesError,
	currentPath,
	navigateTo: navigateToPath,
	editingFile,
	startEditing: (file: EditingFile) => (editingFile.value = file),
	stopEditing: () => (editingFile.value = null),
	createItem: handleCreateItem,
	renameItem: handleRenameItem,
	moveItem: handleMoveItem,
	deleteItem: handleDeleteItem,
	readFile: handleReadFile,
	readFileAsBlob: handleReadFileAsBlob,
	writeFile: handleWriteFile,
	downloadFile: handleDownloadFile,
	uploadFiles: handleUploadFiles,
	uploadState,
	refresh: refreshFiles,
	basePath: serverRoot,
	openInFolder: (path: string) => highlightInFolder(path),
	downloadButtonLabel: formatMessage(messages.saveAs),
})

watch(
	serverId,
	async () => {
		filesFirstPaintPending.value = true
		currentPath.value = ''
		serverRoot.value = await hosting.getDirectory(serverId.value)
		await refreshFiles()
		await refreshContentItems()
	},
	{ immediate: true },
)

useRootBreadcrumb({
	slot: 'root',
	id: 'host',
	label: 'Host',
	to: '/host',
})
</script>

<template>
	<div v-if="server" class="flex flex-col gap-4 p-6">
		<PageHeader :title="server.name">
			<template #leading>
				<Avatar
					:src="hosting.getHostedServerIconUrl(server.icon_path) ?? undefined"
					:alt="server.name"
					size="4rem"
					no-shadow
				/>
			</template>
			<template #metadata>
				<PageHeaderMetadata>
					<PageHeaderMetadataItem :icon="BoxIcon">
						{{ server.loader }} {{ server.game_version }}
					</PageHeaderMetadataItem>
					<PageHeaderMetadataItem :icon="running ? PlayIcon : StopCircleIcon">
						{{ running ? formatMessage(messages.running) : formatMessage(messages.stopped) }}
					</PageHeaderMetadataItem>
				</PageHeaderMetadata>
			</template>
			<template #actions>
				<PageHeaderActions>
					<Button v-if="!running" color="brand" :disabled="busy || !eulaAccepted" @click="handleStart">
						<SpinnerIcon v-if="busy" class="animate-spin" />
						<PlayIcon v-else />
						{{ formatMessage(messages.start) }}
					</Button>
					<Button v-else color="red" :disabled="busy" @click="handleStop">
						<SpinnerIcon v-if="busy" class="animate-spin" />
						<StopCircleIcon v-else />
						{{ formatMessage(messages.stop) }}
					</Button>
					<IconButton :label="formatMessage(messages.settings)" @click="settingsModal?.show()">
						<SettingsIcon />
					</IconButton>
					<TeleportOverflowMenu label="More options" :options="overflowOptions">
						<MoreVerticalIcon />
					</TeleportOverflowMenu>
				</PageHeaderActions>
			</template>
		</PageHeader>

		<NavTabs
			mode="local"
			:links="tabs"
			:active-index="activeTabIndex"
			@tab-click="(index) => (activeTab = tabOrder[index])"
		/>

		<Admonition v-if="!eulaAccepted" type="warning" :header="formatMessage(messages.eulaNotice)">
			<label class="flex items-center gap-2">
				<input
					type="checkbox"
					:checked="eulaAccepted"
					@change="(e) => toggleEula((e.target as HTMLInputElement).checked)"
				/>
				{{ formatMessage(messages.acceptEula) }}
			</label>
		</Admonition>

		<div v-show="activeTab === 'content'" class="flex flex-col gap-4">
			<div class="flex justify-end gap-2">
				<Button
					type="outlined"
					size="lg"
					:disabled="uploadingContent"
					@click="uploadContentFiles"
				>
					<SpinnerIcon v-if="uploadingContent" class="animate-spin" />
					<FolderOpenIcon v-else class="size-5" />
					{{ formatMessage(messages.uploadFiles) }}
				</Button>
				<Button type="colored" color="brand" size="lg" @click="browseContent">
					<CompassIcon class="size-5" />
					{{ formatMessage(messages.browseContent) }}
				</Button>
			</div>

			<ContentCardTable
				v-if="contentItems.length > 0"
				:items="contentItems"
				@delete="handleDeleteContentItem"
			/>
			<EmptyState v-else-if="!contentItemsLoading" type="empty-inbox">
				<template #heading>{{ formatMessage(messages.noContent) }}</template>
			</EmptyState>
		</div>

		<div v-show="activeTab === 'files'" class="flex min-h-[500px] flex-col">
			<ReadyTransition :pending="filesFirstPaintPending">
				<FilePageLayout :show-refresh-button="true" />
			</ReadyTransition>
		</div>

		<div v-show="activeTab === 'console'" class="flex flex-col gap-2">
			<div ref="consoleEl" class="h-96 overflow-y-auto rounded-2xl bg-surface-1 p-3 font-mono text-sm">
				<div v-for="(line, index) in logs" :key="index" class="whitespace-pre-wrap text-secondary">
					{{ line }}
				</div>
			</div>
			<div class="flex gap-2">
				<StyledInput
					v-model="commandInput"
					:placeholder="formatMessage(messages.commandPlaceholder)"
					:disabled="!running"
					class="flex-1"
					@keyup.enter="sendCommand"
				/>
				<Button color="brand" :disabled="!running" @click="sendCommand">
					{{ formatMessage(messages.sendCommand) }}
				</Button>
			</div>
		</div>

		<NewModal ref="settingsModal" :header="formatMessage(messages.settingsModalTitle)">
			<div class="flex w-[24rem] max-w-full flex-col gap-3">
				<div class="flex items-center gap-3">
					<Avatar
						:src="hosting.getHostedServerIconUrl(server.icon_path) ?? undefined"
						:alt="server.name"
						size="3rem"
						no-shadow
					/>
					<div class="flex gap-2">
						<Button type="outlined" :disabled="changingIcon" @click="changeIcon">
							<SpinnerIcon v-if="changingIcon" class="animate-spin" />
							{{ formatMessage(messages.changeIcon) }}
						</Button>
						<IconButton
							v-if="server.icon_path"
							:label="formatMessage(messages.removeIcon)"
							:disabled="changingIcon"
							@click="removeIcon"
						>
							<TrashIcon />
						</IconButton>
					</div>
				</div>
				<div class="grid grid-cols-2 gap-2">
					<label class="flex flex-col gap-1 text-sm text-secondary">
						{{ formatMessage(messages.portLabel) }}
						<StyledInput v-model.number="portInput" type="number" />
					</label>
					<label class="flex flex-col gap-1 text-sm text-secondary">
						{{ formatMessage(messages.memoryLabel) }}
						<StyledInput v-model.number="memoryInput" type="number" />
					</label>
				</div>
				<label class="flex flex-col gap-1 text-sm text-secondary">
					{{ formatMessage(messages.extraArgsLabel) }}
					<StyledInput v-model="extraArgsInput" />
				</label>
			</div>
			<template #actions>
				<div class="flex justify-end">
					<Button color="brand" @click="saveSettings">
						{{ formatMessage(messages.saveSettings) }}
					</Button>
				</div>
			</template>
		</NewModal>
	</div>
</template>
