<script setup lang="ts">
import {
	Checkbox,
	defineMessages,
	injectNotificationManager,
	StyledInput,
	useVIntl,
} from '@modrinth/ui'
import { computed, ref, watch } from 'vue'

import { edit } from '@/helpers/instance'

import type { ModJarPatch } from '../../../../helpers/types'
import { injectInstanceSettings } from './instance-settings-context'

const { handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()

const { instance } = injectInstanceSettings()

const overrideQuickPlay = ref(!!instance.value.quick_play_singleplayer_world)
const quickPlayWorld = ref(instance.value.quick_play_singleplayer_world ?? '')

const overrideJarPatches = ref((instance.value.jar_patches?.length ?? 0) > 0)
const jarPatchesRaw = ref(JSON.stringify(instance.value.jar_patches ?? [], null, 2))
const jarPatchesError = ref(false)

function parseJarPatches(): ModJarPatch[] | null {
	try {
		const parsed = JSON.parse(jarPatchesRaw.value)
		if (!Array.isArray(parsed)) return null
		return parsed as ModJarPatch[]
	} catch {
		return null
	}
}

const parsedJarPatches = computed(() => parseJarPatches())

watch(jarPatchesRaw, () => {
	jarPatchesError.value = overrideJarPatches.value && parsedJarPatches.value === null
})

const editInstanceObject = computed(() => {
	const payload: Record<string, unknown> = {
		quick_play_singleplayer_world:
			overrideQuickPlay.value && quickPlayWorld.value ? quickPlayWorld.value : null,
	}
	if (!overrideJarPatches.value) {
		payload.jar_patches = []
	} else if (parsedJarPatches.value !== null) {
		payload.jar_patches = parsedJarPatches.value
	}
	return payload
})

watch(
	[overrideQuickPlay, quickPlayWorld, overrideJarPatches, parsedJarPatches],
	async () => {
		if (overrideJarPatches.value && parsedJarPatches.value === null) {
			// Invalid JSON: don't overwrite the instance's saved patches with
			// something unparseable. The textarea stays flagged as an error
			// until it's fixed.
			return
		}
		await edit(instance.value.id, editInstanceObject.value).catch(handleError)
	},
	{ deep: true },
)

const messages = defineMessages({
	title: {
		id: 'instance.settings.tabs.patches.title',
		defaultMessage: 'Compatibility patches',
	},
	description: {
		id: 'instance.settings.tabs.patches.description',
		defaultMessage:
			'Advanced, per-instance workarounds for broken or unusual mods. Patches are reapplied automatically whenever the jar or this configuration changes, and are always rebuilt from a kept original, so they stay reversible.',
	},
	quickPlayTitle: {
		id: 'instance.settings.tabs.patches.quick-play.title',
		defaultMessage: 'Default to singleplayer',
	},
	quickPlayCheckbox: {
		id: 'instance.settings.tabs.patches.quick-play.checkbox',
		defaultMessage: 'Always quick-play into a singleplayer world',
	},
	quickPlayDescription: {
		id: 'instance.settings.tabs.patches.quick-play.description',
		defaultMessage:
			"When launching this instance without explicitly joining a server, boot straight into this world instead of stopping at the title screen. Useful when a mod's default menu is a multiplayer front door you don't want to go through. Creates the world if it doesn't exist yet.",
	},
	quickPlayPlaceholder: {
		id: 'instance.settings.tabs.patches.quick-play.placeholder',
		defaultMessage: 'World name',
	},
	jarPatchesTitle: {
		id: 'instance.settings.tabs.patches.jar-patches.title',
		defaultMessage: 'Mod jar patches',
	},
	jarPatchesCheckbox: {
		id: 'instance.settings.tabs.patches.jar-patches.checkbox',
		defaultMessage: 'Patch mod jars before launch',
	},
	jarPatchesDescription: {
		id: 'instance.settings.tabs.patches.jar-patches.description',
		defaultMessage:
			'A JSON array of patches, each targeting one jar in this instance\'s mods folder by file name. "replace_entries" overwrites files inside the jar wholesale (entry path inside the jar → absolute path to read the replacement from). "remove_mixins" strips class names out of a SpongePowered Mixin config file inside the jar (entry path → list of names to remove).',
	},
	jarPatchesExample: {
		id: 'instance.settings.tabs.patches.jar-patches.example',
		defaultMessage: 'Example: patch one broken mixin out of a mod',
	},
})

const exampleJson = JSON.stringify(
	[
		{
			jar_file_name: 'examplemod.jar',
			remove_mixins: {
				'examplemod.mixins.json': ['SomeBrokenMixin'],
			},
		},
	],
	null,
	2,
)
</script>

<template>
	<div>
		<h2 class="m-0 text-lg font-extrabold text-contrast block">
			{{ formatMessage(messages.title) }}
		</h2>
		<p class="mt-1 mb-4">
			{{ formatMessage(messages.description) }}
		</p>

		<h3 class="m-0 mb-1 text-md font-semibold text-contrast block">
			{{ formatMessage(messages.quickPlayTitle) }}
		</h3>
		<Checkbox
			v-model="overrideQuickPlay"
			:label="formatMessage(messages.quickPlayCheckbox)"
			class="my-2"
		/>
		<StyledInput
			id="quick-play-world"
			v-model="quickPlayWorld"
			autocomplete="off"
			:disabled="!overrideQuickPlay"
			:placeholder="formatMessage(messages.quickPlayPlaceholder)"
			wrapper-class="w-full"
		/>
		<p class="mt-1 mb-4">
			{{ formatMessage(messages.quickPlayDescription) }}
		</p>

		<h3 class="m-0 mb-1 text-md font-semibold text-contrast block">
			{{ formatMessage(messages.jarPatchesTitle) }}
		</h3>
		<Checkbox
			v-model="overrideJarPatches"
			:label="formatMessage(messages.jarPatchesCheckbox)"
			class="my-2"
		/>
		<StyledInput
			id="jar-patches"
			v-model="jarPatchesRaw"
			multiline
			:rows="10"
			resize="vertical"
			:disabled="!overrideJarPatches"
			:error="jarPatchesError"
			wrapper-class="w-full font-mono"
		/>
		<p class="mt-1 mb-2">
			{{ formatMessage(messages.jarPatchesDescription) }}
		</p>
		<p class="m-0 font-semibold">{{ formatMessage(messages.jarPatchesExample) }}</p>
		<pre class="mt-1 mb-0 p-3 bg-bg rounded-2xl overflow-x-auto text-sm">{{ exampleJson }}</pre>
	</div>
</template>
