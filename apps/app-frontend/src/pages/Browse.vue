<script setup lang="ts">
import type { Labrinth } from '@modrinth/api-client'
import {
	CheckIcon,
	ClipboardCopyIcon,
	CompassIcon,
	ExternalIcon,
	GlobeIcon,
	PlusIcon,
	ServerStackIcon,
	SpinnerIcon,
} from '@modrinth/assets'
import type { BrowseInstallContentType, CardAction, ProjectType, Tags } from '@modrinth/ui'
import {
	BrowsePageLayout,
	BrowseSidebar,
	commonMessages,
	CreationFlowModal,
	defineMessages,
	formatProjectTypeSentence,
	getLatestMatchingInstallVersion,
	getLoaderMessage,
	getSelectedInstallPreferences,
	getTargetInstallPreferences,
	injectNotificationManager,
	preferencesDiffer,
	provideBrowseManager,
	requestInstall,
	resolveInstallPlan,
	stripServerRuntimeInstallFilters,
	stripServerRuntimeInstallOverrides,
	useBrowseSearch,
	useDebugLogger,
	useVIntl,
} from '@modrinth/ui'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import type { Ref } from 'vue'
import { computed, onBeforeUnmount, ref, shallowRef, watch } from 'vue'
import type { LocationQuery } from 'vue-router'
import { useRoute, useRouter } from 'vue-router'

import ContextMenu from '@/components/ui/context-menu/index.vue'
import { useAppServerBrowse } from '@/composables/browse/use-app-server-browse'
import { useAppEvent } from '@/composables/use-app-event'
import { useAppSettings } from '@/composables/use-app-settings.ts'
import { get_project, get_search_results_v3, get_version, get_version_many } from '@/helpers/cache.js'
import * as curseforge from '@/helpers/curseforge.ts'
import { classIdForProjectType } from '@/helpers/curseforge.ts'
import type { CfMod } from '@/helpers/curseforge.ts'
import * as hosting from '@/helpers/hosting'
import {
	install_create_modpack_instance,
	installJobInstanceId,
} from '@/helpers/install'
import {
	get_installed_project_ids as getInstalledProjectIds,
	getInstanceIconUrl,
	install_curseforge_project_with_dependencies,
	list as listInstances,
} from '@/helpers/instance'
import { get_loader_versions as getLoaderManifest } from '@/helpers/metadata'
import { get as getSettings, set as setSettings } from '@/helpers/settings.ts'
import { get_categories, get_game_versions, get_loaders } from '@/helpers/tags'
import { get_instance_worlds } from '@/helpers/worlds'
import {
	instanceDetailQueryOptions,
	instanceKeys,
	instanceLinkedProjectQueryOptions,
} from '@/pages/instance/query-options'
import { type BreadcrumbDefinition, injectBreadcrumbManager } from '@/providers/breadcrumbs'
import { injectContentInstall } from '@/providers/content-install'
import { injectServerInstall } from '@/providers/server-install'
import {
	createServerInstallContent,
	provideServerInstallContent,
} from '@/providers/setup/server-install-content'

const { handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()
const { installingServerProjects, playServerProject, showAddServerToInstanceModal } =
	injectServerInstall()
const { install: installVersion } = injectContentInstall()
const queryClient = useQueryClient()
const debugLog = useDebugLogger('Browse')

const router = useRouter()
const route = useRoute()
const displayedBrowseRoute = shallowRef(router.currentRoute.value)
watch(
	() => router.currentRoute.value,
	(nextRoute) => {
		if (nextRoute.path.startsWith('/browse/')) {
			displayedBrowseRoute.value = nextRoute
		}
	},
	{ immediate: true },
)
const breadcrumbMessages = defineMessages({
	discoverProjectType: {
		id: 'app.browse.discover-project-type',
		defaultMessage: 'Discover {projectType}',
	},
	discoverServers: {
		id: 'app.browse.discover-servers',
		defaultMessage: 'Discover servers',
	},
})
const breadcrumbLabel = computed(() => {
	const browseRoute = displayedBrowseRoute.value
	if (browseRoute.query.from === 'worlds' || browseRoute.params.projectType === 'server') {
		return formatMessage(breadcrumbMessages.discoverServers)
	}

	return formatMessage(breadcrumbMessages.discoverProjectType, {
		projectType: formatProjectTypeSentence(
			formatMessage,
			String(browseRoute.params.projectType ?? ''),
			2,
		),
	})
})
const appSettings = useAppSettings()
const browseRouteActive = computed(() => route.path.startsWith('/browse/'))
// Set (via a dedicated /browse-host/:projectType route, see routes.js) when
// the Host page's "Choose a modpack" flow sends the user here to pick a
// modpack for a hosted server rather than a client instance -- hides the
// project-type tabs (only modpacks make sense to host) and redirects
// modpack installs to hosting.create() instead of the normal instance-create
// flow. A route.meta flag (set on the matched route record itself) rather
// than a query param, since that's resolved statically at route-match time
// and can't be dropped by any navigation-string parsing quirk.
const isHostPickerContext = computed(() => route.meta.forHost === true)

// Set (via a `hs` query param) when Browse is opened from an *existing*
// hosted server's Content tab "Browse content" button (see host/Detail.vue)
// -- unlike isHostPickerContext (which creates a whole new server from a
// modpack), this installs a single mod/resourcepack/datapack/shaderpack
// directly into that server's own content folder.
const hostedServerId = computed(() => String(route.query.hs ?? ''))
const isHostContentContext = computed(() => !!hostedServerId.value)
const hostedServer = ref<hosting.HostedServer | null>(null)
watch(
	hostedServerId,
	async (id) => {
		hostedServer.value = id ? await hosting.get(id).catch(() => null) : null
	},
	{ immediate: true },
)

const serverSetupModalRef = ref<InstanceType<typeof CreationFlowModal> | null>(null)
const serverInstallContent = createServerInstallContent({ serverSetupModalRef })
provideServerInstallContent(serverInstallContent)
const {
	serverIdQuery,
	serverFlowFrom,
	isFromWorlds,
	isServerContext,
	isSetupServerContext,
	effectiveServerWorldId,
	serverContextServerData,
	serverContentProjectIds,
	queuedServerInstallRootProjectIds,
	queuedServerInstallProjectIds,
	queuedServerInstallCount,
	selectedServerInstallProjects,
	isInstallingQueuedServerInstalls,
	queuedInstallProgress,
	serverBackUrl,
	serverBackLabel,
	serverBrowseHeading,
	clearQueuedServerInstalls,
	removeQueuedServerInstall,
	flushQueuedServerInstalls,
	discardQueuedServerInstallsAndBack,
	installQueuedServerInstallsAndBack,
	initServerContext,
	watchServerContextChanges,
	searchServerModpacks,
	getServerProjectVersions,
	enforceSetupModpackRoute,
	getQueuedServerInstallPlans,
	setQueuedServerInstallPlans,
	resolveQueuedServerInstallPlan,
	openServerModpackInstallFlow,
	onServerFlowBack,
	handleServerModpackFlowCreate,
	markServerProjectInstalled,
} = serverInstallContent

const initialInstanceId = computed(() => String(route.query.i ?? ''))
const instanceQuery = useQuery(
	computed(() => ({
		...instanceDetailQueryOptions(initialInstanceId.value),
		enabled: !!initialInstanceId.value,
	})),
)
const instance = computed(() => instanceQuery.data.value ?? null)
const linkedInstanceProjectId = computed(() => instance.value?.link?.project_id ?? '')
const linkedInstanceProjectQuery = useQuery(
	computed(() => ({
		...instanceLinkedProjectQueryOptions(linkedInstanceProjectId.value),
		enabled: !!linkedInstanceProjectId.value,
	})),
)
const installedProjectIds: Ref<string[] | null> = ref(null)
const instanceHideInstalled = ref(route.query.ai === 'true')
const newlyInstalled = ref<string[]>([])
const hiddenInstanceProjectIds = ref<Set<string>>(new Set())
const hiddenInstanceProjectIdsInitialized = ref(false)
const isServerInstance = computed(
	() => linkedInstanceProjectQuery.data.value?.minecraft_server != null,
)

const breadcrumbManager = injectBreadcrumbManager()
const instanceBreadcrumbDefinition = {
	slot: 'instance',
	id: () => `instance:${String(displayedBrowseRoute.value.query.i ?? '')}`,
	label: () => instance.value?.name ?? formatMessage(commonMessages.loadingLabel),
	visual: () => ({
		type: 'image' as const,
		src: getInstanceIconUrl(instance.value?.icon_path),
		alt: instance.value?.name,
		tintBy: String(displayedBrowseRoute.value.query.i ?? ''),
	}),
	to: () => {
		const instancePath = `/instance/${encodeURIComponent(
			String(displayedBrowseRoute.value.query.i ?? ''),
		)}`
		return displayedBrowseRoute.value.query.from === 'worlds'
			? `${instancePath}/worlds`
			: instancePath
	},
} satisfies BreadcrumbDefinition
const serversBreadcrumbDefinition = {
	slot: 'root',
	id: 'servers',
	label: () => formatMessage(commonMessages.serversLabel),
	to: '/hosting/manage/',
	visual: { type: 'icon', component: ServerStackIcon },
} satisfies BreadcrumbDefinition
const serverBreadcrumbTo = ref(serverBackUrl.value)
watch(serverBackUrl, (value) => {
	if (route.path.startsWith('/browse/')) {
		serverBreadcrumbTo.value = value
	}
})
const serverBreadcrumbDefinition = {
	slot: 'server',
	id: () => `server:${String(displayedBrowseRoute.value.query.sid ?? '')}`,
	label: () => serverContextServerData.value?.name ?? formatMessage(commonMessages.loadingLabel),
	visual: { type: 'icon', component: ServerStackIcon },
	to: serverBreadcrumbTo,
} satisfies BreadcrumbDefinition
const breadcrumbDefinition = {
	slot: 'browse',
	id: () =>
		`browse:${String(displayedBrowseRoute.value.params.projectType ?? '')}:${String(
			displayedBrowseRoute.value.query.i ?? '',
		)}:${String(displayedBrowseRoute.value.query.sid ?? '')}:${String(
			displayedBrowseRoute.value.query.from ?? '',
		)}`,
	label: breadcrumbLabel,
	to: () => displayedBrowseRoute.value.fullPath,
	visual: { type: 'icon', component: CompassIcon },
} satisfies BreadcrumbDefinition

function syncBreadcrumbs() {
	if (displayedBrowseRoute.value.query.i) {
		const instanceBreadcrumb = breadcrumbManager.reset(instanceBreadcrumbDefinition)
		breadcrumbManager.push(breadcrumbDefinition, { parent: instanceBreadcrumb })
		return
	}

	if (displayedBrowseRoute.value.query.sid) {
		const serversBreadcrumb = breadcrumbManager.reset(serversBreadcrumbDefinition)
		const serverBreadcrumb = breadcrumbManager.push(serverBreadcrumbDefinition, {
			parent: serversBreadcrumb,
		})
		breadcrumbManager.push(breadcrumbDefinition, { parent: serverBreadcrumb })
		return
	}

	breadcrumbManager.reset(breadcrumbDefinition)
}

watch(displayedBrowseRoute, syncBreadcrumbs, { immediate: true, flush: 'sync' })

debugLog('fetching tags (categories, loaders, gameVersions)')
const [categories, loaders, availableGameVersions] = await Promise.all([
	get_categories()
		.catch(handleError)
		.then(ref<Labrinth.Tags.v2.Category[]>),
	get_loaders()
		.catch(handleError)
		.then(ref<Labrinth.Tags.v2.Loader[]>),
	get_game_versions()
		.catch(handleError)
		.then(ref<Labrinth.Tags.v2.GameVersion[]>),
])

const tags: Ref<Tags> = computed(() => ({
	gameVersions: availableGameVersions.value ?? [],
	loaders: loaders.value ?? [],
	categories: categories.value ?? [],
}))

if (isFromWorlds.value && route.params.projectType !== 'server') {
	router.replace({
		path: '/browse/server',
		query: route.query,
	})
}

enforceSetupModpackRoute(route.params.projectType as string | undefined)

const allInstalledIds = computed(
	() => new Set([...newlyInstalled.value, ...(installedProjectIds.value ?? [])]),
)

function syncHiddenInstanceProjectIds() {
	hiddenInstanceProjectIds.value = new Set([
		...(installedProjectIds.value ?? []),
		...newlyInstalled.value,
	])
	hiddenInstanceProjectIdsInitialized.value = true
}

watch(
	installedProjectIds,
	(ids) => {
		if (!ids) return
		if (!hiddenInstanceProjectIdsInitialized.value) {
			syncHiddenInstanceProjectIds()
		}
	},
	{ immediate: true },
)

watchServerContextChanges()

await initInstanceContext()

async function refreshInstalledProjectIds() {
	if (!route.query.i) {
		const instances = await queryClient
			.fetchQuery({
				queryKey: [...instanceKeys.all, 'installed-project-ids'],
				queryFn: listInstances,
				staleTime: 0,
			})
			.catch(handleError)
		if (!instances) return

		const ids = instances
			.map((gameInstance) => gameInstance.link?.project_id)
			.filter((id): id is string => !!id)
		debugLog('installedInstanceProjectIds loaded', { count: ids.length })
		installedProjectIds.value = ids
		return
	}

	if (route.query.from === 'worlds') {
		const targetInstanceId = route.query.i as string
		const worlds = await queryClient
			.fetchQuery({
				queryKey: instanceKeys.installedProjectIds(targetInstanceId, 'worlds'),
				queryFn: () => get_instance_worlds(targetInstanceId),
				staleTime: 0,
			})
			.catch(handleError)
		if (!worlds) return

		const serverProjectIds = worlds
			.filter((w) => w.type === 'server' && 'project_id' in w && w.project_id)
			.map((w) => (w as { project_id: string }).project_id)
		debugLog('installedServerProjectIds loaded', { count: serverProjectIds.length })
		installedProjectIds.value = serverProjectIds
		return
	}

	const targetInstanceId = route.query.i as string
	const ids = await queryClient
		.fetchQuery({
			queryKey: instanceKeys.installedProjectIds(targetInstanceId, 'content'),
			queryFn: () => getInstalledProjectIds(targetInstanceId),
			staleTime: 0,
		})
		.catch(handleError)
	if (!ids) return

	debugLog('installedProjectIds loaded', { count: ids.length })
	installedProjectIds.value = ids
}

async function initInstanceContext() {
	debugLog('initInstanceContext', {
		queryI: route.query.i,
		queryAi: route.query.ai,
		querySid: route.query.sid,
		queryWid: route.query.wid,
		queryFrom: route.query.from,
	})
	await Promise.all([
		initServerContext(),
		refreshInstalledProjectIds(),
		route.query.i ? instanceQuery.suspense().catch(handleError) : Promise.resolve(),
	])

	if (route.query.i) {
		debugLog('instance loaded', {
			name: instance.value?.name,
			loader: instance.value?.loader,
			gameVersion: instance.value?.game_version,
		})

		if (instance.value?.link?.project_id) {
			await linkedInstanceProjectQuery.suspense().catch(handleError)
		}
	}
}

function setBrowseHideInstalledFlag(flag: 'hide_installed_modpacks', value: boolean) {
	appSettings.featureFlags[flag] = value
	getSettings()
		.then((settings) => {
			settings.feature_flags[flag] = value
			return setSettings(settings)
		})
		.catch(handleError)
}

const hideInstalledModpacks = computed({
	get: () => appSettings.getFeatureFlag('hide_installed_modpacks'),
	set: (value: boolean) => setBrowseHideInstalledFlag('hide_installed_modpacks', value),
})

const instanceFilters = computed(() => {
	const filters = []

	if (instance.value && projectType.value !== 'resourcepack') {
		const isVanillaShader = projectType.value === 'shader' && instance.value.loader === 'vanilla'
		const gameVersion = instance.value.game_version
		if (gameVersion && !isVanillaShader) {
			filters.push({ type: 'game_version', option: gameVersion })
		}

		const platform = instance.value.loader
		const supportedModLoaders = ['fabric', 'forge', 'quilt', 'neoforge']

		if (platform && projectType.value === 'mod' && supportedModLoaders.includes(platform)) {
			filters.push({ type: 'mod_loader', option: platform })
		}
		if (isVanillaShader) {
			filters.push({ type: 'shader_loader', option: 'vanilla' })
		}

		if (isServerInstance.value) {
			filters.push({ type: 'environment', option: 'client' })
		}
	}

	if (
		(instance.value || projectType.value === 'modpack') &&
		(projectType.value === 'modpack' ? hideInstalledModpacks.value : instanceHideInstalled.value) &&
		hiddenInstanceProjectIds.value.size > 0
	) {
		for (const id of hiddenInstanceProjectIds.value) {
			filters.push({ type: 'project_id', option: `project_id:${id}`, negative: true })
		}
	}

	return filters
})

const serverHideInstalled = ref(false)
const hideSelectedServerInstalls = ref(false)
if (route.query.shi) {
	serverHideInstalled.value = route.query.shi === 'true'
}
const hiddenServerContentProjectIds = ref<Set<string>>(new Set())
const hiddenServerContentProjectIdsInitialized = ref(false)

function syncHiddenServerContentProjectIds() {
	hiddenServerContentProjectIds.value = new Set(serverContentProjectIds.value)
	hiddenServerContentProjectIdsInitialized.value = true
}

watch(
	serverContentProjectIds,
	() => {
		if (!hiddenServerContentProjectIdsInitialized.value) {
			syncHiddenServerContentProjectIds()
		}
	},
	{ immediate: true },
)

const serverContextFilters = computed(() => {
	const filters: { type: string; option: string; negative?: boolean }[] = []
	if (!serverContextServerData.value) return filters
	const pt = projectType.value

	if (pt !== 'modpack') {
		const gameVersion = serverContextServerData.value.mc_version
		if (gameVersion) filters.push({ type: 'game_version', option: gameVersion })

		const platform = serverContextServerData.value.loader?.toLowerCase()
		if (platform && ['fabric', 'forge', 'quilt', 'neoforge'].includes(platform))
			filters.push({ type: 'mod_loader', option: platform })
		if (platform && ['paper', 'purpur'].includes(platform))
			filters.push({ type: 'plugin_loader', option: platform })

		if (pt === 'mod') filters.push({ type: 'environment', option: 'server' })

		if (hideSelectedServerInstalls.value && queuedServerInstallProjectIds.value.size > 0) {
			for (const id of queuedServerInstallProjectIds.value) {
				filters.push({ type: 'project_id', option: `project_id:${id}`, negative: true })
			}
		}
	}

	if (pt === 'modpack') {
		filters.push(
			{ type: 'environment', option: 'client' },
			{ type: 'environment', option: 'server' },
		)

		if (hideInstalledModpacks.value && hiddenInstanceProjectIds.value.size > 0) {
			for (const id of hiddenInstanceProjectIds.value) {
				filters.push({ type: 'project_id', option: `project_id:${id}`, negative: true })
			}
		}
	}

	if (serverHideInstalled.value && hiddenServerContentProjectIds.value.size > 0) {
		for (const id of hiddenServerContentProjectIds.value) {
			filters.push({ type: 'project_id', option: `project_id:${id}`, negative: true })
		}
	}

	return filters
})

const combinedProvidedFilters = computed(() =>
	isServerContext.value ? serverContextFilters.value : instanceFilters.value,
)

const {
	serverPings,
	contextMenuRef,
	updateServerHits,
	getServerModpackContent,
	getServerCardActions,
	handleRightClick,
	handleOptionsClick,
} = useAppServerBrowse({
	instance,
	isFromWorlds,
	allInstalledIds,
	newlyInstalled,
	installingServerProjects,
	playServerProject,
	showAddServerToInstanceModal,
	handleError,
	router,
})

const offline = ref(!navigator.onLine)
const handleOffline = () => {
	debugLog('went offline')
	offline.value = true
}
const handleOnline = () => {
	debugLog('went online')
	offline.value = false
}
window.addEventListener('offline', handleOffline)
window.addEventListener('online', handleOnline)

onBeforeUnmount(() => {
	window.removeEventListener('offline', handleOffline)
	window.removeEventListener('online', handleOnline)
})

const messages = defineMessages({
	addServersToInstance: {
		id: 'app.browse.add-servers-to-instance',
		defaultMessage: 'Adding server to instance',
	},
	addToAnInstance: {
		id: 'app.browse.add-to-an-instance',
		defaultMessage: 'Add to an instance',
	},
	environmentProvidedByServer: {
		id: 'search.filter.locked.server-environment.title',
		defaultMessage: 'Only client-side mods can be added to the server instance',
	},
	gameVersionProvidedByInstance: {
		id: 'search.filter.locked.instance-game-version.title',
		defaultMessage: 'Game version is provided by the instance',
	},
	hideAddedServers: {
		id: 'app.browse.hide-added-servers',
		defaultMessage: 'Hide servers already added',
	},
	hideInstalledModpacks: {
		id: 'app.browse.hide-installed-modpacks',
		defaultMessage: 'Hide already installed',
	},
	installingToServer: {
		id: 'app.browse.server.installing',
		defaultMessage: 'Installing',
	},
	backToInstance: {
		id: 'app.browse.back-to-instance',
		defaultMessage: 'Back to instance',
	},
	serverInstanceContentWarning: {
		id: 'app.browse.server-instance-content-warning',
		defaultMessage:
			'Adding content may prevent you from joining this server. Any content you add will be removed when the managed server content is updated.',
	},
	modLoaderProvidedByInstance: {
		id: 'search.filter.locked.instance-loader.title',
		defaultMessage: 'Loader is provided by the instance',
	},
	modpacksProjectType: {
		id: 'app.browse.project-type.modpacks',
		defaultMessage: 'Modpacks',
	},
	modsProjectType: { id: 'app.browse.project-type.mods', defaultMessage: 'Mods' },
	resourcePacksProjectType: {
		id: 'app.browse.project-type.resource-packs',
		defaultMessage: 'Resource Packs',
	},
	dataPacksProjectType: {
		id: 'app.browse.project-type.data-packs',
		defaultMessage: 'Data Packs',
	},
	shadersProjectType: { id: 'app.browse.project-type.shaders', defaultMessage: 'Shaders' },
	serversProjectType: { id: 'app.browse.project-type.servers', defaultMessage: 'Servers' },
	providedByInstance: {
		id: 'search.filter.locked.instance',
		defaultMessage: 'Provided by the instance',
	},
	syncFilterButton: {
		id: 'search.filter.locked.instance.sync',
		defaultMessage: 'Sync with instance',
	},
})

const projectType = ref<ProjectType>(route.params.projectType as ProjectType)

function resetInstanceContext() {
	debugLog('instance context removed, resetting')
	installedProjectIds.value = null
	instanceHideInstalled.value = false
	newlyInstalled.value = []
	hiddenInstanceProjectIds.value = new Set()
	hiddenInstanceProjectIdsInitialized.value = false
	isServerInstance.value = false
	browseBreadcrumb.reset()
	void refreshInstalledProjectIds()
}

watch(
	() => route.params.projectType as ProjectType,
	async (newType) => {
		if (!browseRouteActive.value) {
			return
		}
		if (isSetupServerContext.value) {
			enforceSetupModpackRoute(newType)
			if (newType !== 'modpack') return
		}

		if (!newType || newType === projectType.value) return

		debugLog('projectType route param changed', { from: projectType.value, to: newType })
		projectType.value = newType
	},
)

watch(
	() => route.query.i,
	async (nextInstanceId, previousInstanceId) => {
		if (!route.path.startsWith('/browse') || nextInstanceId === previousInstanceId) return
		if (!nextInstanceId) {
			resetInstanceContext()
			return
		}

		installedProjectIds.value = null
		hiddenInstanceProjectIdsInitialized.value = false
		await Promise.all([instanceQuery.suspense().catch(handleError), refreshInstalledProjectIds()])
		if (instance.value?.link?.project_id) {
			await linkedInstanceProjectQuery.suspense().catch(handleError)
		}
	},
)

const selectableProjectTypes = computed(() => {
	let dataPacks = false,
		mods = false,
		modpacks = false

	if (instance.value) {
		if (
			availableGameVersions.value &&
			availableGameVersions.value.findIndex((x) => x.version === instance.value?.game_version) <=
				availableGameVersions.value.findIndex((x) => x.version === '1.13') &&
			!isServerInstance.value
		) {
			dataPacks = true
		}

		if (instance.value.loader !== 'vanilla') {
			mods = true
		}
	} else {
		dataPacks = true
		mods = true
		modpacks = true
	}

	const params: LocationQuery = {}

	if (route.query.i) params.i = route.query.i
	if (route.query.ai) params.ai = route.query.ai
	if (route.query.from) params.from = route.query.from
	if (route.query.sid) params.sid = route.query.sid
	if (effectiveServerWorldId.value) params.wid = effectiveServerWorldId.value

	const queryString = new URLSearchParams(params as Record<string, string>).toString()
	const suffix = queryString ? `?${queryString}` : ''

	if (isSetupServerContext.value) {
		return [
			{ label: formatMessage(messages.modpacksProjectType), href: `/browse/modpack${suffix}` },
		]
	}

	if (isFromWorlds.value) {
		return [{ label: formatMessage(messages.serversProjectType), href: `/browse/server${suffix}` }]
	}

	return [
		{
			label: formatMessage(messages.modpacksProjectType),
			href: `/browse/modpack${suffix}`,
			shown: modpacks,
		},
		{ label: formatMessage(messages.modsProjectType), href: `/browse/mod${suffix}`, shown: mods },
		{
			label: formatMessage(messages.resourcePacksProjectType),
			href: `/browse/resourcepack${suffix}`,
		},
		{
			label: formatMessage(messages.dataPacksProjectType),
			href: `/browse/datapack${suffix}`,
			shown: dataPacks,
		},
		{ label: formatMessage(messages.shadersProjectType), href: `/browse/shader${suffix}` },
		{
			label: formatMessage(messages.serversProjectType),
			href: `/browse/server${suffix}`,
			shown: !instance.value,
		},
	]
})

const installContext = computed(() => {
	if (isServerContext.value && serverContextServerData.value) {
		return {
			name: serverContextServerData.value.name,
			loader: serverContextServerData.value.loader ?? '',
			gameVersion: serverContextServerData.value.mc_version ?? '',
			serverId: serverIdQuery.value,
			upstream: serverContextServerData.value.upstream,
			iconSrc: null as string | null,
			isMedal: serverContextServerData.value.is_medal,
			backUrl: serverBackUrl.value,
			backLabel: serverBackLabel.value,
			heading: serverBrowseHeading.value,
			queuedCount: queuedServerInstallCount.value,
			selectedProjects: selectedServerInstallProjects.value,
			isInstallingSelected: isInstallingQueuedServerInstalls.value,
			skipNonEssentialWarnings: appSettings.getFeatureFlag('skip_non_essential_warnings'),
			installProgress: queuedInstallProgress.value,
			clearQueued: clearQueuedServerInstalls,
			clearSelected: clearQueuedServerInstalls,
			onBack: flushQueuedServerInstalls,
			discardSelectedAndBack: discardQueuedServerInstallsAndBack,
			installSelected: installQueuedServerInstallsAndBack,
		}
	}
	if (instance.value) {
		return {
			name: instance.value.name,
			loader: instance.value.loader,
			gameVersion: instance.value.game_version,
			iconSrc: getInstanceIconUrl(instance.value.icon_path),
			backUrl: `/instance/${encodeURIComponent(instance.value.id)}${isFromWorlds.value ? '/worlds' : ''}`,
			backLabel: formatMessage(messages.backToInstance),
			heading: formatMessage(
				isFromWorlds.value ? messages.addServersToInstance : commonMessages.installingContentLabel,
			),
			warning:
				isServerInstance.value && instance.value.loader !== 'vanilla' && !isFromWorlds.value
					? formatMessage(messages.serverInstanceContentWarning)
					: undefined,
		}
	}
	return null
})

const installingProjectIds = ref<Set<string>>(new Set())

function setProjectInstalling(projectId: string, installing: boolean) {
	const next = new Set(installingProjectIds.value)
	if (installing) {
		next.add(projectId)
	} else {
		next.delete(projectId)
	}
	installingProjectIds.value = next
}

const serverInstallQueue = {
	get: getQueuedServerInstallPlans,
	set: setQueuedServerInstallPlans,
}

function getCurrentSelectedInstallPreferences(projectTypeValue: string) {
	return getSelectedInstallPreferences({
		contentType: projectTypeValue,
		selectedFilters: searchState.currentFilters.value,
		providedFilters: combinedProvidedFilters.value,
		overriddenProvidedFilterTypes: searchState.overriddenProvidedFilterTypes.value,
	})
}

function getServerInstallTargetPreferences(contentType: BrowseInstallContentType) {
	return getTargetInstallPreferences(
		{
			gameVersion: serverContextServerData.value?.mc_version,
			loader: serverContextServerData.value?.loader,
		},
		contentType,
	)
}

function getInstanceInstallTargetPreferences(projectTypeValue: string) {
	return getTargetInstallPreferences(
		{
			gameVersion: instance.value?.game_version,
			loader: instance.value?.loader,
		},
		projectTypeValue,
	)
}

async function getInstallProjectVersions(projectId: string) {
	const project = await get_project(projectId, 'must_revalidate')
	return (await get_version_many(
		project.versions,
		'must_revalidate',
	)) as Labrinth.Versions.v2.Version[]
}

async function chooseInstanceInstallVersion(
	project: Labrinth.Search.v3.ResultSearchProject,
	projectTypeValue: string,
) {
	const targetInstance = instance.value
	if (!targetInstance) {
		return { versionId: null as string | null }
	}

	const selectedPreferences = getCurrentSelectedInstallPreferences(projectTypeValue)
	const targetPreferences = getInstanceInstallTargetPreferences(projectTypeValue)
	if (!preferencesDiffer(selectedPreferences, targetPreferences)) {
		return { versionId: null as string | null }
	}

	const selectedVersion = getLatestMatchingInstallVersion(
		await getInstallProjectVersions(project.project_id),
		selectedPreferences,
	)

	if (!selectedVersion) {
		return { versionId: null as string | null }
	}

	return { versionId: selectedVersion.id }
}

async function chooseFilterMatchingInstallVersion(
	project: Labrinth.Search.v3.ResultSearchProject,
	projectTypeValue: string,
) {
	const plan = await resolveInstallPlan({
		project: {
			project_id: project.project_id,
			title: project.title,
			icon_url: project.icon_url,
		},
		contentType: projectTypeValue as BrowseInstallContentType,
		selectedFilters: searchState.currentFilters.value,
		providedFilters: combinedProvidedFilters.value,
		overriddenProvidedFilterTypes: searchState.overriddenProvidedFilterTypes.value,
		targetPreferences: {},
		getProjectVersions: getInstallProjectVersions,
	})

	return { versionId: plan.versionId }
}

// When Browse is opened from the Host page's "Choose a modpack" flow
// (isHostPickerContext), a modpack install creates a hosted server instead
// of a client instance -- this bypasses installVersion()/
// install_create_modpack_instance entirely (same reasoning as the
// CurseForge modpack bypass below: those calls are hard-wired to the client
// instance-creation pipeline).
async function installAsHostedServer(
	name: string,
	source: Parameters<typeof hosting.create>[1],
) {
	const server = await hosting.create(name, source)
	await router.push(`/host/${server.id}`)
}

// Single-project installs into an *existing* hosted server (isHostContentContext)
// -- unlike installAsHostedServer above, no new server is created. Hosted
// servers don't track content by project/hash the way client instances do
// (see hosting/install.rs's module doc), so this just resolves one version's
// primary file and downloads it straight into the right content folder.
function contentDirForProjectType(projectType: string): string | null {
	switch (projectType) {
		case 'mod':
		case 'plugin':
			return 'mods'
		case 'resourcepack':
			return 'resourcepacks'
		case 'datapack':
			return 'datapacks'
		case 'shader':
			return 'shaderpacks'
		default:
			return null
	}
}

async function chooseHostedServerInstallVersion(
	project: Labrinth.Search.v3.ResultSearchProject,
	projectTypeValue: string,
) {
	const plan = await resolveInstallPlan({
		project: {
			project_id: project.project_id,
			title: project.title,
			icon_url: project.icon_url,
		},
		contentType: projectTypeValue as BrowseInstallContentType,
		selectedFilters: searchState.currentFilters.value,
		providedFilters: combinedProvidedFilters.value,
		overriddenProvidedFilterTypes: searchState.overriddenProvidedFilterTypes.value,
		targetPreferences: getTargetInstallPreferences(
			{
				gameVersion: hostedServer.value?.game_version,
				loader: hostedServer.value?.loader,
			},
			projectTypeValue,
		),
		getProjectVersions: getInstallProjectVersions,
	})

	return { versionId: plan.versionId }
}

async function installModrinthFileToHostedServer(versionId: string, currentProjectType: string) {
	const contentDir = contentDirForProjectType(currentProjectType)
	if (!contentDir || !hostedServerId.value) {
		throw new Error(`Can't install ${currentProjectType} content into a hosted server`)
	}
	const version = (await get_version(versionId, 'must_revalidate')) as Labrinth.Versions.v2.Version
	const file = version.files.find((f) => f.primary) ?? version.files[0]
	if (!file) {
		throw new Error('This version has no downloadable files')
	}
	await hosting.installModrinthFile(
		hostedServerId.value,
		contentDir,
		file.filename,
		file.url,
		file.hashes?.sha1 ?? null,
	)
}

async function installCurseForgeFileToHostedServer(fileId: string, currentProjectType: string) {
	const contentDir = contentDirForProjectType(currentProjectType)
	if (!contentDir || !hostedServerId.value) {
		throw new Error(`Can't install ${currentProjectType} content into a hosted server`)
	}
	await hosting.installCurseForgeFile(hostedServerId.value, contentDir, fileId)
}

// Used only as a fallback for a merged provider entry (see mergeProviderHits)
// when installing its Modrinth version actually threw. Leaves file_id unset
// so the backend resolver picks the file matching the instance's loader and
// game version, rather than blindly installing CurseForge's own
// unsorted/unfiltered `latestFiles[0]`.
async function installCurseForgeFallback(mod: CfMod, currentProjectType: string) {
	if (mod.allowModDistribution === false) {
		throw new Error(
			`The author of "${mod.name}" has disabled third-party downloads for this mod on CurseForge.`,
		)
	}
	if (!instance.value) {
		throw new Error(`No instance to install "${mod.name}" into`)
	}
	await install_curseforge_project_with_dependencies(instance.value.id, {
		mod_id: mod.id.toString(),
		file_id: null,
		content_type: currentProjectType as Labrinth.Content.v3.ContentType,
	})
}

const curseforgeInstalling = ref<Set<string>>(new Set())
const curseforgeInstalled = ref<Set<string>>(new Set())

// CurseForge modpacks create a whole new instance rather than installing
// content into an existing one, so they go through install_create_modpack_instance
// directly instead of the shared Modrinth installVersion()/content-install.ts
// flow (which resolves projects via get_project(), a Modrinth-only cache
// lookup that doesn't know about "curseforge:"-prefixed project ids).
function getCurseForgeModpackCardActions(
	result: Labrinth.Search.v3.ResultSearchProject & CurseForgeTaggedHit,
): CardAction[] {
	const mod = result.__curseforge

	if (mod.allowModDistribution === false) {
		return [
			{
				key: 'install',
				label: 'Not available via 3rd party',
				icon: PlusIcon,
				disabled: true,
				color: 'red',
				type: 'outlined',
				onClick: async () => {},
			},
		]
	}

	const isInstalling = curseforgeInstalling.value.has(result.project_id)

	return [
		{
			key: 'install',
			label: formatMessage(
				isInstalling ? commonMessages.installingLabel : commonMessages.installButton,
			),
			icon: isInstalling ? SpinnerIcon : PlusIcon,
			iconClass: isInstalling ? 'animate-spin' : undefined,
			disabled: isInstalling,
			color: 'brand',
			type: 'outlined',
			onClick: async () => {
				const file = mod.latestFiles[0]
				if (!file) {
					handleError(new Error(`No files available for "${mod.name}" on CurseForge`))
					return
				}

				curseforgeInstalling.value = new Set([...curseforgeInstalling.value, result.project_id])
				try {
					if (isHostPickerContext.value) {
						await installAsHostedServer(mod.name, {
							type: 'curseforge_modpack',
							mod_id: mod.id.toString(),
							file_id: file.id.toString(),
						})
						return
					}

					const job = await install_create_modpack_instance({
						type: 'fromCurseForgeFile',
						mod_id: mod.id.toString(),
						file_id: file.id.toString(),
						title: mod.name,
						icon_url: mod.logo?.url ?? null,
					})
					const newInstanceId = installJobInstanceId(job)
					if (newInstanceId) {
						router.push(`/instance/${newInstanceId}`)
					}
				} catch (err) {
					handleError(err as Error)
				} finally {
					const next = new Set(curseforgeInstalling.value)
					next.delete(result.project_id)
					curseforgeInstalling.value = next
				}
			},
		},
	]
}

// Mirrors the loader-alias handling in modrinth-content-management's version
// resolver (packages/modrinth-content-management/src/install.rs) so a
// client-side CurseForge file pick agrees with what the backend resolver
// would choose for an instance install.
const CURSEFORGE_LOADER_ALIAS_GROUPS = [['neoforge', 'neo']]

function normalizeCfLoaderAlias(loader: string): string {
	return loader.toLowerCase().replaceAll('_', '').replaceAll('-', '').replaceAll(' ', '')
}

function cfLoaderAliases(loader: string): Set<string> {
	const normalized = normalizeCfLoaderAlias(loader)
	const aliases = new Set([normalized])
	const group = CURSEFORGE_LOADER_ALIAS_GROUPS.find((g) => g.includes(normalized))
	if (group) {
		for (const alias of group) aliases.add(alias)
	}
	return aliases
}

function cfFileMatchesTarget(file: CfFile, gameVersion: string, loader: string): boolean {
	const tags = file.gameVersions.map((v) => v.toLowerCase())
	if (!tags.includes(gameVersion.toLowerCase())) return false
	const aliases = cfLoaderAliases(loader)
	return tags.some((tag) => aliases.has(normalizeCfLoaderAlias(tag)))
}

// Only used for the hosted-server install path, which (unlike installing
// into a client instance) has no backend resolver to fall back on --
// hosting_install_curseforge_file just downloads whatever file id it's
// given. mod.latestFiles is CurseForge's own unsorted "one recent file per
// loader/version" summary, so pick the file actually matching the server's
// loader/game version from the full file list instead of grabbing
// latestFiles[0], which can easily be for a different loader/MC version.
async function pickCurseForgeFileForHostedServer(mod: CfMod, fallback: CfFile): Promise<CfFile> {
	const gameVersion = hostedServer.value?.game_version
	const loader = hostedServer.value?.loader
	if (!gameVersion || !loader) return fallback
	try {
		const files = await curseforge.getModFiles(mod.id.toString())
		return files.find((file) => cfFileMatchesTarget(file, gameVersion, loader)) ?? fallback
	} catch {
		return fallback
	}
}

function getCurseForgeCardActions(
	result: Labrinth.Search.v3.ResultSearchProject & CurseForgeTaggedHit,
	currentProjectType: string,
): CardAction[] {
	const mod = result.__curseforge
	const canInstall = !!instance.value || isHostContentContext.value

	if (mod.allowModDistribution === false) {
		return [
			{
				key: 'install',
				label: 'Not available via 3rd party',
				icon: PlusIcon,
				disabled: true,
				color: 'red',
				type: 'outlined',
				onClick: async () => {},
			},
		]
	}

	const isInstalled = curseforgeInstalled.value.has(result.project_id)
	const isInstalling = curseforgeInstalling.value.has(result.project_id)

	return [
		{
			key: 'install',
			label: formatMessage(
				isInstalled
					? commonMessages.installedLabel
					: canInstall
						? commonMessages.installButton
						: messages.addToAnInstance,
			),
			icon: isInstalling ? SpinnerIcon : isInstalled ? CheckIcon : PlusIcon,
			iconClass: isInstalling ? 'animate-spin' : undefined,
			disabled: isInstalled || isInstalling || !canInstall,
			color: 'brand',
			type: 'outlined',
			onClick: async () => {
				if (!instance.value && !isHostContentContext.value) return
				const file = mod.latestFiles[0]
				if (!file) {
					handleError(new Error(`No files available for "${mod.name}" on CurseForge`))
					return
				}

				curseforgeInstalling.value = new Set([...curseforgeInstalling.value, result.project_id])
				try {
					if (isHostContentContext.value) {
						const target = await pickCurseForgeFileForHostedServer(mod, file)
						await installCurseForgeFileToHostedServer(target.id.toString(), currentProjectType)
					} else if (instance.value) {
						// Leave file_id unset so the backend resolver picks the file
						// matching this instance's game version and mod loader,
						// instead of blindly installing latestFiles[0] (CurseForge's
						// own summary list, which is not filtered for the current
						// instance at all -- e.g. a mod's Forge 1.12.2 file can sort
						// before its NeoForge 1.20.1 file).
						await install_curseforge_project_with_dependencies(instance.value.id, {
							mod_id: mod.id.toString(),
							file_id: null,
							content_type: currentProjectType as Labrinth.Content.v3.ContentType,
						})
					}
					curseforgeInstalled.value = new Set([...curseforgeInstalled.value, result.project_id])
					onSearchResultInstalled(result.project_id)
				} catch (err) {
					handleError(err as Error)
				} finally {
					const next = new Set(curseforgeInstalling.value)
					next.delete(result.project_id)
					curseforgeInstalling.value = next
				}
			},
		},
	]
}

function getCardActions(
	result: Labrinth.Search.v3.ResultSearchProject,
	currentProjectType: string,
): CardAction[] {
	if (isCurseForgeHit(result)) {
		return currentProjectType === 'modpack'
			? getCurseForgeModpackCardActions(result)
			: getCurseForgeCardActions(result, currentProjectType)
	}

	if (currentProjectType === 'server') {
		return getServerCardActions(result)
	}

	const projectResult = result as Labrinth.Search.v3.ResultSearchProject & {
		installed?: boolean
		installing?: boolean
	}
	const isInstalled =
		projectResult.installed ||
		allInstalledIds.value.has(projectResult.project_id || '') ||
		serverContentProjectIds.value.has(projectResult.project_id || '') ||
		serverContextServerData.value?.upstream?.project_id === projectResult.project_id
	const isInstalling = installingProjectIds.value.has(projectResult.project_id)
	const showAsInstalled = isInstalled && currentProjectType !== 'modpack'

	if (
		isServerContext.value &&
		['modpack', 'mod', 'plugin', 'datapack'].includes(currentProjectType)
	) {
		const isQueued = queuedServerInstallProjectIds.value.has(projectResult.project_id)
		const isQueuedRoot = queuedServerInstallRootProjectIds.value.has(projectResult.project_id)
		const isInstallingSelection = isInstallingQueuedServerInstalls.value
		const validatingInstall =
			isInstalling && currentProjectType !== 'modpack' && !isInstallingSelection
		const installLabel = showAsInstalled
			? commonMessages.installedLabel
			: isQueued
				? isInstalling || isInstallingSelection
					? validatingInstall
						? commonMessages.validatingLabel
						: messages.installingToServer
					: commonMessages.selectedLabel
				: isInstalling || isInstallingSelection
					? validatingInstall
						? commonMessages.validatingLabel
						: messages.installingToServer
					: commonMessages.installButton
		return [
			{
				key: 'install',
				label: formatMessage(installLabel),
				icon:
					isInstalling || isInstallingSelection
						? SpinnerIcon
						: isQueued || showAsInstalled
							? CheckIcon
							: PlusIcon,
				iconClass: isInstalling || isInstallingSelection ? 'animate-spin' : undefined,
				disabled:
					showAsInstalled || isInstalling || isInstallingSelection || (isQueued && !isQueuedRoot),
				color: isQueued && !isInstalling && !isInstallingSelection ? 'green' : 'brand',
				type: 'outlined',
				onClick: async () => {
					if (isQueuedRoot) {
						removeQueuedServerInstall(projectResult.project_id)
						return
					}
					if (isQueued) return

					const contentType = currentProjectType as BrowseInstallContentType
					const isModpack = contentType === 'modpack'
					const shouldShowInstalling = isModpack || !isQueued
					if (shouldShowInstalling) {
						setProjectInstalling(projectResult.project_id, true)
					}
					try {
						const plan = await requestInstall({
							project: projectResult,
							contentType,
							mode: isModpack ? 'immediate' : 'queue',
							selectedFilters: isModpack
								? []
								: stripServerRuntimeInstallFilters(searchState.currentFilters.value),
							providedFilters: isModpack ? [] : combinedProvidedFilters.value,
							overriddenProvidedFilterTypes: isModpack
								? []
								: stripServerRuntimeInstallOverrides(
										searchState.overriddenProvidedFilterTypes.value,
									),
							targetPreferences: getServerInstallTargetPreferences(contentType),
							getProjectVersions: getInstallProjectVersions,
							queue: serverInstallQueue,
							install: (plan) =>
								openServerModpackInstallFlow({
									projectId: plan.projectId,
									versionId: plan.versionId,
									name: plan.project.name,
									iconUrl: plan.project.icon_url ?? undefined,
								}),
						})
						if (!isModpack) await resolveQueuedServerInstallPlan(plan)
					} catch (err) {
						if (!isModpack) removeQueuedServerInstall(projectResult.project_id)
						handleError(err as Error)
					} finally {
						if (shouldShowInstalling) {
							setProjectInstalling(projectResult.project_id, false)
						}
					}
				},
			},
		]
	}

	const isModpack = projectResult.project_types?.includes('modpack')
	const shouldUseInstallIcon = !!instance.value || isModpack || isHostContentContext.value

	return [
		{
			key: 'install',
			label: formatMessage(
				isInstalling
					? messages.installingToServer
					: showAsInstalled
						? commonMessages.installedLabel
						: shouldUseInstallIcon
							? commonMessages.installButton
							: messages.addToAnInstance,
			),
			icon: isInstalling ? SpinnerIcon : showAsInstalled ? CheckIcon : PlusIcon,
			iconClass: isInstalling ? 'animate-spin' : undefined,
			disabled: showAsInstalled || isInstalling,
			color: 'brand',
			type: 'outlined',
			onClick: async () => {
				setProjectInstalling(projectResult.project_id, true)
				try {
					if (isHostPickerContext.value && isModpack) {
						const selected = await chooseFilterMatchingInstallVersion(
							projectResult,
							currentProjectType,
						)
						if (selected === null || !selected.versionId) {
							setProjectInstalling(projectResult.project_id, false)
							return
						}
						await installAsHostedServer(projectResult.name, {
							type: 'modrinth_modpack',
							project_id: projectResult.project_id,
							version_id: selected.versionId,
						})
						setProjectInstalling(projectResult.project_id, false)
						return
					}

					if (isHostContentContext.value && !isModpack) {
						const selected = await chooseHostedServerInstallVersion(
							projectResult,
							currentProjectType,
						)
						if (selected === null || !selected.versionId) {
							setProjectInstalling(projectResult.project_id, false)
							return
						}
						try {
							await installModrinthFileToHostedServer(selected.versionId, currentProjectType)
							onSearchResultInstalled(projectResult.project_id)
						} finally {
							setProjectInstalling(projectResult.project_id, false)
						}
						return
					}

					const selectedInstall = instance.value
						? await chooseInstanceInstallVersion(projectResult, currentProjectType)
						: isModpack
							? await chooseFilterMatchingInstallVersion(projectResult, currentProjectType)
							: { versionId: null as string | null }
					if (selectedInstall === null) {
						setProjectInstalling(projectResult.project_id, false)
						return
					}
					const selectedPreferences = getCurrentSelectedInstallPreferences(currentProjectType)
					await installVersion(
						projectResult.project_id,
						selectedInstall.versionId,
						instance.value ? instance.value.id : null,
						'SearchCard',
						(versionId, installedProjectIds) => {
							setProjectInstalling(projectResult.project_id, false)
							if (versionId) {
								onSearchResultsInstalled(installedProjectIds ?? [projectResult.project_id])
							}
						},
						(profile) => {
							router.push(`/instance/${profile}`)
						},
						{
							preferredLoader: instance.value?.loader ?? selectedPreferences.loaders?.[0],
							preferredGameVersion:
								instance.value?.game_version ?? selectedPreferences.gameVersions?.[0],
						},
					)
				} catch (err) {
					if (
						instance.value &&
						!isHostPickerContext.value &&
						!isHostContentContext.value &&
						!isModpack &&
						hasCurseForgeFallback(projectResult)
					) {
						try {
							await installCurseForgeFallback(
								projectResult.__curseforgeFallback,
								currentProjectType,
							)
							onSearchResultInstalled(projectResult.project_id)
						} catch (fallbackErr) {
							handleError(fallbackErr as Error)
						} finally {
							setProjectInstalling(projectResult.project_id, false)
						}
						return
					}

					setProjectInstalling(projectResult.project_id, false)
					handleError(err)
				}
			},
		},
	]
}

function onSearchResultInstalled(id: string) {
	if (isServerContext.value) {
		markServerProjectInstalled(id)
		return
	}
	if (!newlyInstalled.value.includes(id)) {
		newlyInstalled.value = [...newlyInstalled.value, id]
	}
}

function onSearchResultsInstalled(ids: string[]) {
	if (isServerContext.value) {
		for (const id of ids) {
			markServerProjectInstalled(id)
		}
		return
	}
	newlyInstalled.value = Array.from(new Set([...newlyInstalled.value, ...ids]))
}

interface CurseForgeTaggedHit {
	__curseforge: CfMod
}

function isCurseForgeHit(
	result: Labrinth.Search.v3.ResultSearchProject,
): result is Labrinth.Search.v3.ResultSearchProject & CurseForgeTaggedHit {
	return !!result.project_id?.startsWith('curseforge:')
}

// Attached to a Modrinth hit when a CurseForge mod with the same name+author
// was found and folded into it (see mergeProviderHits below), so the browse
// grid shows one card instead of two duplicate entries for the same mod.
// Installing that card still installs the Modrinth version first; this is
// only consulted as a fallback if the Modrinth install actually fails.
interface CurseForgeFallbackHit {
	__curseforgeFallback: CfMod
}

function hasCurseForgeFallback(
	result: Labrinth.Search.v3.ResultSearchProject,
): result is Labrinth.Search.v3.ResultSearchProject & CurseForgeFallbackHit {
	return !!(result as Partial<CurseForgeFallbackHit>).__curseforgeFallback
}

function normalizeForMerge(value: string): string {
	return value.trim().toLowerCase()
}

// Modrinth's `author` field on a search hit is the project owner's
// *username*; CurseForge's is whatever *display name* they registered with
// on that platform. These are frequently different strings for the exact
// same person/mod (e.g. a Modrinth handle vs. a CurseForge display name), so
// requiring an exact match here was silently preventing almost every real
// merge. Treat them as "the same author" if they match after normalizing,
// or either one contains the other.
function authorsLooselyMatch(a: string, b: string): boolean {
	const na = normalizeForMerge(a)
	const nb = normalizeForMerge(b)
	if (!na || !nb) return false
	return na === nb || na.includes(nb) || nb.includes(na)
}

// Folds CurseForge hits into their Modrinth twin so the grid shows one
// merged card instead of two duplicate entries for the same mod. Matching is
// name-first: the same mod name on both platforms is already a strong
// signal by itself. Author is only consulted to disambiguate when more than
// one CurseForge hit shares that exact name (a generic name like "Storage"
// published independently by unrelated authors on each platform) -- if it
// can't be disambiguated, no merge happens rather than guessing wrong.
// Modpacks are excluded: they create a whole new instance on install via
// provider-specific pipelines (install_create_modpack_instance vs.
// install_create_instance) that can't share a single fallback path the way
// mod/resourcepack/datapack/shader installs into an existing instance can.
function mergeProviderHits(
	modrinthHits: Labrinth.Search.v3.ResultSearchProject[],
	curseforgeHits: (Labrinth.Search.v3.ResultSearchProject & CurseForgeTaggedHit)[],
	projectTypeValue: string,
): {
	mergedModrinthHits: Labrinth.Search.v3.ResultSearchProject[]
	remainingCurseForgeHits: (Labrinth.Search.v3.ResultSearchProject & CurseForgeTaggedHit)[]
} {
	if (projectTypeValue === 'modpack' || curseforgeHits.length === 0) {
		return { mergedModrinthHits: modrinthHits, remainingCurseForgeHits: curseforgeHits }
	}

	const curseforgeByName = new Map<string, (typeof curseforgeHits)[number][]>()
	for (const cfHit of curseforgeHits) {
		const key = normalizeForMerge(cfHit.name)
		const list = curseforgeByName.get(key)
		if (list) {
			list.push(cfHit)
		} else {
			curseforgeByName.set(key, [cfHit])
		}
	}

	const usedCurseForgeIds = new Set<number>()
	const mergedModrinthHits = modrinthHits.map((hit) => {
		const candidates = curseforgeByName.get(normalizeForMerge(hit.name))
		if (!candidates || candidates.length === 0) return hit

		const cfMatch =
			candidates.length === 1
				? candidates[0]
				: candidates.find((candidate) => authorsLooselyMatch(hit.author, candidate.author))
		if (!cfMatch) return hit

		usedCurseForgeIds.add(cfMatch.__curseforge.id)
		const merged: Labrinth.Search.v3.ResultSearchProject & CurseForgeFallbackHit = {
			...hit,
			__curseforgeFallback: cfMatch.__curseforge,
		}
		return merged
	})

	const remainingCurseForgeHits = curseforgeHits.filter(
		(cfHit) => !usedCurseForgeIds.has(cfHit.__curseforge.id),
	)

	return { mergedModrinthHits, remainingCurseForgeHits }
}

// CurseForge's gameVersions array mixes Minecraft version numbers, loader
// names, environment tags, and other version-ish labels together (e.g.
// ["1.20.1", "Fabric", "Client", "Beta 1.7.3", "Java 17"]) with no way to
// tell them apart by shape alone -- "Beta 1.7.3" doesn't start with a digit
// either. Only treat an entry as a loader if it's a name Modrinth's own
// loader vocabulary recognizes, rather than guessing from what it *isn't*.
function cfLoaders(mod: CfMod): string[] {
	const loaders = new Set<string>()
	for (const file of mod.latestFiles) {
		for (const gameVersion of file.gameVersions) {
			const normalized = gameVersion.toLowerCase()
			if (getLoaderMessage(normalized)) {
				loaders.add(normalized)
			}
		}
	}
	return [...loaders]
}

// Mirrors Modrinth's project_loader_fields.environment, derived from
// CurseForge's "Client"/"Server" gameVersions tags, so CurseForge cards get
// the same environment badge (rendered before the tag row) as Modrinth ones.
function cfEnvironment(mod: CfMod): Labrinth.Projects.v3.Environment | undefined {
	let hasClient = false
	let hasServer = false
	for (const file of mod.latestFiles) {
		for (const gameVersion of file.gameVersions) {
			const normalized = gameVersion.toLowerCase()
			if (normalized === 'client') hasClient = true
			if (normalized === 'server') hasServer = true
		}
	}
	if (hasClient && hasServer) return 'client_and_server'
	if (hasClient) return 'client_only'
	if (hasServer) return 'server_only'
	return undefined
}

function cfModToSearchHit(
	mod: CfMod,
	projectType: string,
): Labrinth.Search.v3.ResultSearchProject & CurseForgeTaggedHit {
	// Only the main category is shown (no sub-categories), matching the
	// Modrinth-side mapping below: source, [environment,] main category,
	// loaders -- not a full category list.
	const mainCategory = mod.categories[0]?.name.toLowerCase()
	const categories = ['curseforge', ...(mainCategory ? [mainCategory] : [])]
	// The search request itself was already scoped to this project type's
	// classId, so every hit in the response is one -- no need to trust
	// `mod.classId` alone (CurseForge sometimes omits it on search results).
	const projectTypes = [projectType]

	return {
		project_id: `curseforge:${mod.id}`,
		project_types: projectTypes,
		all_project_types: projectTypes,
		slug: null,
		author: mod.authors[0]?.name ?? 'CurseForge',
		author_id: null,
		organization: null,
		organization_id: null,
		name: mod.name,
		summary: mod.summary,
		categories,
		display_categories: categories,
		downloads: mod.downloadCount ?? 0,
		// The follower system is being removed from this app entirely
		// (CurseForge has no equivalent concept anyway); the shared
		// ProjectCardStats component only renders this stat when defined.
		follows: undefined as unknown as number,
		icon_url: mod.logo?.url ?? null,
		date_created: mod.dateCreated ?? new Date(0).toISOString(),
		date_modified: mod.dateModified ?? new Date(0).toISOString(),
		license: 'unknown',
		gallery: [],
		featured_gallery: null,
		color: null,
		project_loader_fields: (() => {
			const environment = cfEnvironment(mod)
			return environment ? { environment: [environment] } : undefined
		})(),
		loaders: cfLoaders(mod),
		disclosure_types: [],
		__curseforge: mod,
	}
}

function selectedProviders(): Set<'modrinth' | 'curseforge'> {
	const sourceFilters = searchState.currentFilters.value.filter(
		(f) => f.type === 'source' && !f.negative,
	)
	if (sourceFilters.length === 0) return new Set(['modrinth', 'curseforge'])
	return new Set(sourceFilters.map((f) => f.option as 'modrinth' | 'curseforge'))
}

// Modrinth and CurseForge each return their own hits pre-sorted server-side
// (by whichever sort each provider's API applies), so merging them requires
// re-sorting the combined list client-side by whatever field the user's
// chosen sort type corresponds to -- otherwise CurseForge results just end
// up appended after every Modrinth result regardless of the selected sort.
// A missing/invalid value must always sort to the bottom of a descending
// sort, never the top -- a plain `?? 0` (or an unguarded Date.parse, which
// returns NaN for an empty string) doesn't guarantee that: NaN comparator
// results are treated as "equal" by Array.prototype.sort, which can leave
// bad data anywhere, including first.
function safeSortValue(n: number): number {
	return Number.isFinite(n) ? n : -Infinity
}

function compareBySelectedSort(
	a: Labrinth.Search.v3.ResultSearchProject,
	b: Labrinth.Search.v3.ResultSearchProject,
): number {
	const sortName = searchState.effectiveCurrentSortType.value.name

	switch (sortName) {
		case 'downloads':
			return safeSortValue(b.downloads ?? NaN) - safeSortValue(a.downloads ?? NaN)
		case 'newest':
			return (
				safeSortValue(Date.parse(b.date_created ?? '')) -
				safeSortValue(Date.parse(a.date_created ?? ''))
			)
		case 'updated':
			return (
				safeSortValue(Date.parse(b.date_modified ?? '')) -
				safeSortValue(Date.parse(a.date_modified ?? ''))
			)
		case 'relevance':
		default:
			// Neither provider's API exposes a directly comparable relevance
			// score across providers, so fall back to downloads as the
			// closest useful proxy for a merged "relevance" ordering.
			return safeSortValue(b.downloads ?? NaN) - safeSortValue(a.downloads ?? NaN)
	}
}

// Modpacks install a whole new instance rather than content into an existing
// one, but they're still discovered through this same search path -- only
// their install action (getCurseForgeModpackCardActions) differs.
const CURSEFORGE_CONTENT_SEARCH_PROJECT_TYPES = [
	'mod',
	'resourcepack',
	'datapack',
	'shader',
	'modpack',
]

async function searchCurseForgeHits(
	requestParams: string,
): Promise<(Labrinth.Search.v3.ResultSearchProject & CurseForgeTaggedHit)[]> {
	// Only on the first page, since CurseForge and Modrinth have independent
	// pagination that isn't merged here.
	if (!CURSEFORGE_CONTENT_SEARCH_PROJECT_TYPES.includes(projectType.value)) return []
	if (!selectedProviders().has('curseforge')) return []
	const classId = classIdForProjectType(projectType.value)
	if (classId === null) return []

	const params = new URLSearchParams(
		requestParams.startsWith('?') ? requestParams.slice(1) : requestParams,
	)
	const queryText = params.get('query') ?? ''
	const offsetParam = params.get('offset')
	if (offsetParam && offsetParam !== '0') return []

	const sortName = searchState.effectiveCurrentSortType.value.name
	const sortField =
		sortName === 'downloads'
			? curseforge.CF_SORT_FIELD.totalDownloads
			: sortName === 'newest'
				? curseforge.CF_SORT_FIELD.releasedDate
				: sortName === 'updated'
					? curseforge.CF_SORT_FIELD.lastUpdated
					: // 'relevance': CurseForge ranks by textual match relevance itself
						// when no sortField is given and there's an actual search term --
						// passing a field like Popularity would override that with
						// something that isn't relevance at all. Only fall back to
						// Popularity for a pure empty-query browse, where "relevance"
						// doesn't mean anything to rank by.
						queryText.trim()
						? null
						: curseforge.CF_SORT_FIELD.popularity

	try {
		const results = await curseforge.search(
			queryText,
			instance.value?.game_version ?? null,
			classId,
			sortField,
			0,
			20,
		)
		return results.hits.map((mod) => cfModToSearchHit(mod, projectType.value))
	} catch (err) {
		debugLog('curseforge search failed', err)
		handleError(err as Error)
		return []
	}
}

async function search(requestParams: string) {
	debugLog('searching v3', requestParams)
	const isServer = projectType.value === 'server'

	const [rawResults, curseforgeHits] = await Promise.all([
		queryClient.fetchQuery({
			queryKey: ['search', 'v3', requestParams],
			queryFn: () =>
				get_search_results_v3(requestParams, 'must_revalidate') as Promise<{
					result: Labrinth.Search.v3.SearchResults & {
						hits: (Labrinth.Search.v3.ResultSearchProject & { installed?: boolean })[]
					}
				} | null>,
			staleTime: 30_000,
		}),
		isServer ? Promise.resolve([]) : searchCurseForgeHits(requestParams),
	])

	if (!rawResults) {
		return {
			projectHits: [],
			serverHits: [],
			total_hits: 0,
			per_page: 20,
		}
	}

	for (const hit of rawResults.result.hits) {
		for (const identifier of [hit.project_id, hit.slug]) {
			if (identifier) {
				queryClient.setQueryData(['projects', 'summary', identifier], hit)
			}
		}
	}

	if (isServer) {
		const hits = rawResults.result.hits ?? []
		updateServerHits(hits)
		return {
			projectHits: [],
			serverHits: hits,
			total_hits: rawResults.result.total_hits ?? 0,
			per_page: rawResults.result.hits_per_page,
		}
	}

	const hits = rawResults.result.hits.map((hit) => {
		const mapped: Labrinth.Search.v3.ResultSearchProject & { installed?: boolean } = {
			...hit,
			// The Rust side passes v3 search hits through as untyped JSON
			// (unvalidated against this TS type), so guarantee these are
			// real numbers before they ever reach the cross-provider sort --
			// an undefined/NaN downloads value must never be able to make a
			// popular Modrinth mod lose to a barely-downloaded CurseForge one.
			downloads: typeof hit.downloads === 'number' ? hit.downloads : 0,
			// The follower system is being removed from this app entirely --
			// the shared ProjectCardStats component only renders this stat
			// when defined.
			follows: undefined as unknown as number,
			// Only the main category is shown (no sub-categories), matching
			// the CurseForge-side mapping: source, [environment,] main
			// category, loaders -- not a full category list.
			categories: ['modrinth', ...(hit.display_categories ?? []).slice(0, 1)],
			display_categories: ['modrinth', ...(hit.display_categories ?? []).slice(0, 1)],
		}

		if (instance.value || isServerContext.value || projectType.value === 'modpack') {
			const installedIds =
				isServerContext.value && projectType.value !== 'modpack'
					? serverContentProjectIds.value
					: new Set([...newlyInstalled.value, ...(installedProjectIds.value ?? [])])
			mapped.installed = installedIds.has(hit.project_id)
		}

		return mapped
	})

	const { mergedModrinthHits, remainingCurseForgeHits } = mergeProviderHits(
		hits,
		curseforgeHits,
		projectType.value,
	)

	const combinedHits = [...mergedModrinthHits, ...remainingCurseForgeHits]
		.filter((hit) => {
			const provider = isCurseForgeHit(hit) ? 'curseforge' : 'modrinth'
			return selectedProviders().has(provider)
		})
		.sort(compareBySelectedSort)

	return {
		projectHits: combinedHits,
		serverHits: [],
		total_hits: rawResults.result.total_hits + remainingCurseForgeHits.length,
		per_page: rawResults.result.hits_per_page,
	}
}

const lockedFilterMessages = computed(() => ({
	gameVersion: formatMessage(messages.gameVersionProvidedByInstance),
	modLoader: formatMessage(messages.modLoaderProvidedByInstance),
	environment: formatMessage(messages.environmentProvidedByServer),
	syncButton: formatMessage(messages.syncFilterButton),
	providedBy: formatMessage(messages.providedByInstance),
}))

const searchState = useBrowseSearch({
	projectType,
	tags,
	active: browseRouteActive,
	providedFilters: combinedProvidedFilters,
	search,
	persistentQueryParams: ['i', 'ai', 'shi', 'sid', 'wid', 'from'],
	getExtraQueryParams: () => ({
		sid: serverIdQuery.value || undefined,
		wid: effectiveServerWorldId.value || undefined,
		ai: instanceHideInstalled.value ? 'true' : undefined,
		shi: serverHideInstalled.value ? 'true' : undefined,
	}),
})

// The 'source' filter (Modrinth/CurseForge) never touches the real Modrinth
// search query string by design -- it's inert to Labrinth's backend, only
// read client-side in search() above. That means the shared search
// composable's requestParams-based refetch watcher never fires for it, so
// force a refresh here whenever the selected sources change.
watch(
	() => searchState.currentFilters.value.filter((f) => f.type === 'source'),
	() => {
		void searchState.refreshSearch()
	},
	{ deep: true },
)

watch(
	[
		() => searchState.query.value,
		() =>
			searchState.isServerType.value
				? searchState.serverCurrentFilters.value
				: searchState.currentFilters.value,
		() => projectType.value,
	],
	() => {
		if (isServerContext.value && projectType.value !== 'modpack') {
			syncHiddenServerContentProjectIds()
		} else if (instance.value || projectType.value === 'modpack') {
			syncHiddenInstanceProjectIds()
		}
	},
	{ deep: true },
)

watch(queuedServerInstallCount, (count) => {
	if (count === 0) {
		hideSelectedServerInstalls.value = false
	}
})

if (instance.value?.game_version) {
	const gv = instance.value.game_version
	const alreadyHasGv = searchState.serverCurrentFilters.value.some(
		(f) => f.type === 'server_game_version' && f.option === gv,
	)
	if (!alreadyHasGv) {
		searchState.serverCurrentFilters.value.push({ type: 'server_game_version', option: gv })
	}
}

void searchState.refreshSearch()

useAppEvent('instance', async (event) => {
	if (event.event === 'created' || event.event === 'removed') {
		if (!route.query.i) {
			await refreshInstalledProjectIds()
			if (projectType.value === 'modpack') {
				if (event.event === 'removed') {
					syncHiddenInstanceProjectIds()
				}
				await searchState.refreshSearch()
			}
		}
	}

	if (instance.value && event.instance_id === instance.value.id && event.event === 'synced') {
		await refreshInstalledProjectIds()
		await searchState.refreshSearch()
	}
})

function getProjectBrowseQuery() {
	if (!browseRouteActive.value) {
		return undefined
	}
	if (!installContext.value) return undefined
	return {
		...route.query,
		b: route.fullPath,
	}
}

const advancedFiltersCollapsed = computed({
	get: () => appSettings.getFeatureFlag('advanced_filters_collapsed'),
	set: (value) => {
		appSettings.featureFlags['advanced_filters_collapsed'] = value
		getSettings()
			.then((settings) => {
				settings.feature_flags['advanced_filters_collapsed'] = value
				return setSettings(settings)
			})
			.catch(handleError)
	},
})

const dismissedPhotosensitivityFilterWarning = computed({
	get: () => appSettings.getFeatureFlag('dismissed_photosensitivity_filter_warning'),
	set: (value) => {
		appSettings.featureFlags['dismissed_photosensitivity_filter_warning'] = value
		getSettings()
			.then((settings) => {
				settings.feature_flags['dismissed_photosensitivity_filter_warning'] = value
				return setSettings(settings)
			})
			.catch(handleError)
	},
})

provideBrowseManager({
	tags,
	projectType,
	...searchState,
	advancedFiltersCollapsed,
	dismissedPhotosensitivityFilterWarning,
	getProjectLink: (result: Labrinth.Search.v3.ResultSearchProject) => {
		if (isCurseForgeHit(result)) {
			return {
				path: `/curseforge-project/${result.__curseforge.id}`,
				query: getProjectBrowseQuery(),
			}
		}
		return {
			path: `/project/${result.project_id ?? result.slug}`,
			query: getProjectBrowseQuery(),
		}
	},
	getServerProjectLink: (result: Labrinth.Search.v3.ResultSearchProject) => ({
		path: `/project/${result.slug ?? result.project_id}`,
		query: getProjectBrowseQuery(),
	}),
	selectableProjectTypes,
	showProjectTypeTabs: computed(() => !isServerContext.value && !isHostPickerContext.value),
	variant: 'app',
	getCardActions,
	installContext,
	providedFilters: combinedProvidedFilters,
	hideInstalled: computed({
		get: () => {
			if (projectType.value === 'modpack') return hideInstalledModpacks.value
			if (isServerContext.value) return serverHideInstalled.value
			return instanceHideInstalled.value
		},
		set: (val: boolean) => {
			if (projectType.value === 'modpack') {
				hideInstalledModpacks.value = val
				if (val) syncHiddenInstanceProjectIds()
				return
			}
			if (isServerContext.value) {
				serverHideInstalled.value = val
				if (val) syncHiddenServerContentProjectIds()
			} else {
				instanceHideInstalled.value = val
				if (val) syncHiddenInstanceProjectIds()
			}
		},
	}),
	showHideInstalled: computed(
		() =>
			projectType.value === 'modpack' ||
			(isServerContext.value && projectType.value !== 'modpack') ||
			!!instance.value,
	),
	hideInstalledLabel: computed(() =>
		formatMessage(
			isFromWorlds.value
				? messages.hideAddedServers
				: projectType.value === 'modpack'
					? messages.hideInstalledModpacks
					: commonMessages.hideInstalledContentLabel,
		),
	),
	hideSelected: hideSelectedServerInstalls,
	showHideSelected: computed(
		() =>
			isServerContext.value &&
			projectType.value !== 'modpack' &&
			queuedServerInstallCount.value > 0,
	),
	hideSelectedLabel: computed(() => formatMessage(commonMessages.hideSelectedContentLabel)),
	onInstalled: onSearchResultInstalled,
	serverPings,
	getServerModpackContent,
	onContextMenu: handleRightClick,
	offline,
	lockedFilterMessages,
})

</script>

<template>
	<div class="flex flex-col gap-3 p-6">
		<BrowsePageLayout>
			<template #after>
				<ContextMenu ref="contextMenuRef" @option-clicked="handleOptionsClick">
					<template #open_link>
						<GlobeIcon /> {{ formatMessage(commonMessages.openInModrinthButton) }} <ExternalIcon />
					</template>
					<template #copy_link>
						<ClipboardCopyIcon /> {{ formatMessage(commonMessages.copyLinkButton) }}
					</template>
				</ContextMenu>
			</template>
		</BrowsePageLayout>
		<CreationFlowModal
			v-if="isServerContext && projectType === 'modpack'"
			ref="serverSetupModalRef"
			:type="serverFlowFrom === 'reset-server' ? 'reset-server' : 'server-onboarding'"
			:available-loaders="['vanilla', 'fabric', 'neoforge', 'forge', 'quilt', 'paper', 'purpur']"
			:show-snapshot-toggle="true"
			:on-back="onServerFlowBack"
			:search-modpacks="searchServerModpacks"
			:get-project-versions="getServerProjectVersions"
			:get-loader-manifest="getLoaderManifest"
			@hide="() => {}"
			@browse-modpacks="() => {}"
			@create="handleServerModpackFlowCreate"
		/>
		<Teleport v-if="browseRouteActive" to="#sidebar-teleport-target">
			<BrowseSidebar />
		</Teleport>
	</div>
</template>
