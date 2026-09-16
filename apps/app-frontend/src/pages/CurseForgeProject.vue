<template>
	<div class="flex flex-col gap-4 p-6">
		<InstanceIndicator v-if="instance" :instance="instance" />
		<template v-if="mod">
			<div class="flex flex-row flex-wrap items-start gap-4">
				<img
					v-if="mod.logo?.url"
					:src="mod.logo.url"
					:alt="mod.name"
					class="h-20 w-20 rounded-[--radius-lg] object-cover"
				/>
				<div class="flex min-w-0 flex-1 flex-col gap-1">
					<h1 class="m-0 text-2xl font-extrabold text-contrast">{{ mod.name }}</h1>
					<p class="m-0 text-secondary">{{ mod.summary }}</p>
					<div class="flex flex-row flex-wrap items-center gap-x-3 gap-y-1 text-sm text-secondary">
						<span v-if="mod.authors.length > 0">
							{{ formatMessage(messages.byAuthors, { authors: authorNames }) }}
						</span>
						<span v-if="mod.downloadCount != null">
							{{
								formatMessage(messages.downloadsCount, { count: formatNumber(mod.downloadCount) })
							}}
						</span>
						<span v-for="category in mod.categories" :key="category.name" class="tag">
							{{ category.name }}
						</span>
					</div>
				</div>
				<div class="flex flex-row flex-wrap items-start gap-2">
					<Button
						type="colored"
						color="brand"
						size="xl"
						native-type="button"
						:disabled="installDisabled"
						@click="handleInstall"
					>
						<component :is="installIcon" :class="installIconClass" />
						{{ installLabel }}
					</Button>
					<ButtonLink :href="curseForgeUrl ?? undefined" target="_blank" size="xl">
						<GlobeIcon /> {{ formatMessage(messages.openOnCurseForge) }} <ExternalIcon />
					</ButtonLink>
				</div>
			</div>

			<div v-if="descriptionHtml" class="markdown-body" v-html="descriptionHtml" />
			<p v-else-if="descriptionLoading" class="text-secondary">
				{{ formatMessage(commonMessages.loadingLabel) }}
			</p>

			<template v-if="mod.screenshots.length > 0">
				<h2 class="m-0 text-lg font-bold text-contrast">
					{{ formatMessage(messages.galleryHeading) }}
				</h2>
				<Gallery :project="galleryProject" />
			</template>
		</template>
		<template v-else-if="loadError">
			{{ formatMessage(messages.loadError) }}
		</template>
	</div>
</template>

<script setup lang="ts">
import type { Labrinth } from '@modrinth/api-client'
import { CheckIcon, ExternalIcon, GlobeIcon, PlusIcon, SpinnerIcon } from '@modrinth/assets'
import {
	Button,
	ButtonLink,
	commonMessages,
	defineMessages,
	injectNotificationManager,
	useFormatNumber,
	useVIntl,
} from '@modrinth/ui'
import { configuredXss } from '@modrinth/utils'
import { computed, ref, shallowRef, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import InstanceIndicator from '@/components/ui/InstanceIndicator.vue'
import type { CfMod } from '@/helpers/curseforge.ts'
import * as curseforge from '@/helpers/curseforge.ts'
import { projectTypeForClassId } from '@/helpers/curseforge.ts'
import { install_create_modpack_instance, installJobInstanceId } from '@/helpers/install'
import {
	get as getInstance,
	get_installed_project_ids as getInstalledProjectIds,
	install_curseforge_project_with_dependencies,
} from '@/helpers/instance'
import Gallery from '@/pages/project/Gallery.vue'

const props = defineProps<{ id: string | number }>()

const { handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()
const formatNumber = useFormatNumber()
const route = useRoute()
const router = useRouter()

const messages = defineMessages({
	byAuthors: {
		id: 'app.curseforge-project.by-authors',
		defaultMessage: 'By {authors}',
	},
	downloadsCount: {
		id: 'app.curseforge-project.downloads-count',
		defaultMessage: '{count} downloads',
	},
	openOnCurseForge: {
		id: 'app.curseforge-project.open-on-curseforge',
		defaultMessage: 'Open on CurseForge',
	},
	galleryHeading: {
		id: 'app.curseforge-project.gallery-heading',
		defaultMessage: 'Gallery',
	},
	loadError: {
		id: 'app.curseforge-project.load-error',
		defaultMessage: 'CurseForge project data could not be loaded.',
	},
	selectInstance: {
		id: 'app.curseforge-project.select-instance',
		defaultMessage: 'Open from an instance to install',
	},
	notAvailable: {
		id: 'app.curseforge-project.not-available',
		defaultMessage: 'Not available via 3rd party',
	},
})

const mod = shallowRef<CfMod | null>(null)
const descriptionHtml = shallowRef<string | null>(null)
const descriptionLoading = ref(false)
const loadError = ref(false)
const instance = shallowRef<{ id: string; [key: string]: unknown } | null>(null)
const installedProjectIds = ref<Set<string>>(new Set())
const installing = ref(false)

const authorNames = computed(() => mod.value?.authors.map((a) => a.name).join(', ') ?? '')
const curseForgeUrl = computed(() => mod.value?.links?.websiteUrl ?? null)

const contentType = computed(() => (mod.value ? projectTypeForClassId(mod.value.classId) : null))

const galleryProject = computed(() => ({
	gallery: (mod.value?.screenshots ?? []).map((screenshot) => ({
		url: screenshot.thumbnailUrl ?? screenshot.url,
		raw_url: screenshot.url,
		title: screenshot.title ?? '',
		description: screenshot.description ?? '',
		created: mod.value?.dateModified ?? mod.value?.dateCreated ?? new Date().toISOString(),
	})),
}))

const isInstalled = computed(
	() => !!mod.value && installedProjectIds.value.has(String(mod.value.id)),
)
const canDistribute = computed(() => mod.value?.allowModDistribution !== false)
const canInstallHere = computed(() => contentType.value === 'modpack' || !!instance.value)

const installDisabled = computed(
	() =>
		!mod.value ||
		!canDistribute.value ||
		isInstalled.value ||
		installing.value ||
		!canInstallHere.value,
)

const installIcon = computed(() => {
	if (installing.value) return SpinnerIcon
	if (isInstalled.value) return CheckIcon
	return PlusIcon
})
const installIconClass = computed(() => (installing.value ? 'animate-spin' : undefined))

const installLabel = computed(() => {
	if (!canDistribute.value) return formatMessage(messages.notAvailable)
	if (isInstalled.value) return formatMessage(commonMessages.installedLabel)
	if (installing.value) return formatMessage(commonMessages.installingLabel)
	if (!canInstallHere.value) return formatMessage(messages.selectInstance)
	return formatMessage(commonMessages.installButton)
})

async function handleInstall() {
	if (!mod.value || installDisabled.value) return

	installing.value = true
	try {
		if (contentType.value === 'modpack') {
			// Modpacks are installed as a specific, self-contained file rather than
			// matched against an instance's loader/game version, so the latest file
			// is the correct (and only sensible) choice here.
			const file = mod.value.latestFiles[0]
			if (!file) {
				handleError(new Error(`No files available for "${mod.value.name}" on CurseForge`))
				return
			}
			const job = await install_create_modpack_instance({
				type: 'fromCurseForgeFile',
				mod_id: mod.value.id.toString(),
				file_id: file.id.toString(),
				title: mod.value.name,
				icon_url: mod.value.logo?.url ?? null,
			})
			const newInstanceId = installJobInstanceId(job)
			if (newInstanceId) {
				router.push(`/instance/${newInstanceId}`)
			}
			return
		}

		if (!instance.value || !contentType.value) return

		// Leave file_id unset so the backend resolver picks the file matching
		// this instance's game version and mod loader, instead of blindly
		// installing latestFiles[0] (CurseForge's own summary list, which is
		// not filtered or sorted for the current instance at all).
		await install_curseforge_project_with_dependencies(instance.value.id, {
			mod_id: mod.value.id.toString(),
			file_id: null,
			content_type: contentType.value as Labrinth.Content.v3.ContentType,
		})
		installedProjectIds.value = new Set([...installedProjectIds.value, String(mod.value.id)])
	} catch (err) {
		handleError(err as Error)
	} finally {
		installing.value = false
	}
}

async function fetchData() {
	const requestedId = String(props.id ?? route.params.id ?? '')
	if (!requestedId) return

	loadError.value = false
	mod.value = null
	descriptionHtml.value = null

	const [fetchedMod, instanceId] = await Promise.all([
		curseforge.getMod(requestedId).catch((err) => {
			handleError(err)
			return null
		}),
		Promise.resolve(typeof route.query.i === 'string' ? route.query.i : null),
	])

	if (String(route.params.id ?? props.id ?? '') !== requestedId) return

	if (!fetchedMod) {
		loadError.value = true
		return
	}
	mod.value = fetchedMod

	if (instanceId) {
		const [inst, ids] = await Promise.all([
			getInstance(instanceId).catch(() => null),
			getInstalledProjectIds(instanceId).catch(() => []),
		])
		instance.value = inst
		installedProjectIds.value = new Set(ids)
	} else {
		instance.value = null
		installedProjectIds.value = new Set()
	}

	descriptionLoading.value = true
	curseforge
		.getModDescription(requestedId)
		.then((html) => {
			if (String(route.params.id ?? props.id ?? '') !== requestedId) return
			descriptionHtml.value = configuredXss.process(html)
		})
		.catch(handleError)
		.finally(() => {
			descriptionLoading.value = false
		})
}

await fetchData()

watch(() => [props.id, route.params.id, route.query.i], fetchData)
</script>

<style scoped lang="scss">
.tag {
	padding: 0.15rem 0.5rem;
	border-radius: var(--radius-max, 1rem);
	background-color: var(--color-button-bg);
	color: var(--color-text);
}
</style>
