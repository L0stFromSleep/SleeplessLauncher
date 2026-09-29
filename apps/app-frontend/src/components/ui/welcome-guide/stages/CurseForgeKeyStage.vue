<script setup lang="ts">
import { ExternalIcon, KeyIcon } from '@modrinth/assets'
import { Button, defineMessages, injectNotificationManager, StyledInput, useVIntl } from '@modrinth/ui'
import { openUrl } from '@tauri-apps/plugin-opener'
import { onMounted, ref, watch } from 'vue'

import { get, set } from '@/helpers/settings.ts'

const { handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()

const messages = defineMessages({
	title: {
		id: 'app.welcome-guide.curseforge.title',
		defaultMessage: 'Connect your CurseForge account',
	},
	description: {
		id: 'app.welcome-guide.curseforge.description',
		defaultMessage:
			"CurseForge requires every app to use its own API key. It's free, takes under a minute, and you only need to do this once.",
	},
	getKeyButton: {
		id: 'app.welcome-guide.curseforge.get-key-button',
		defaultMessage: 'Get your free API key',
	},
	apiKeyPlaceholder: {
		id: 'app.welcome-guide.curseforge.api-key-placeholder',
		defaultMessage: 'Paste your personal CurseForge API key...',
	},
	optionalNote: {
		id: 'app.welcome-guide.curseforge.optional-note',
		defaultMessage: "You can skip this and add it later from Settings → Integrations if you'd rather.",
	},
})

const apiKey = ref<string | null>(null)
const loaded = ref(false)

onMounted(async () => {
	const settings = await get()
	apiKey.value = settings.curseforge_api_key ?? null
	loaded.value = true
})

watch(apiKey, async (value) => {
	if (!loaded.value) return

	const settings = await get()
	settings.curseforge_api_key = value || null
	await set(settings).catch(handleError)
})

function openCurseForgeConsole() {
	void openUrl('https://console.curseforge.com/')
}
</script>

<template>
	<div class="flex flex-col gap-4">
		<div class="flex flex-col gap-2">
			<h2 class="m-0 flex items-center gap-2 text-lg font-semibold text-contrast">
				<KeyIcon aria-hidden="true" class="size-5 shrink-0 text-brand" />
				{{ formatMessage(messages.title) }}
			</h2>
			<p class="m-0 leading-tight text-secondary">
				{{ formatMessage(messages.description) }}
			</p>
			<Button type="outlined" class="w-max" @click="openCurseForgeConsole">
				<ExternalIcon aria-hidden="true" />
				{{ formatMessage(messages.getKeyButton) }}
			</Button>
		</div>
		<div class="h-px w-full bg-surface-5" />
		<StyledInput
			id="welcome-guide-curseforge-api-key"
			v-model="apiKey"
			autocomplete="off"
			type="password"
			:placeholder="formatMessage(messages.apiKeyPlaceholder)"
			wrapper-class="w-full"
		/>
		<p class="m-0 text-sm leading-tight text-secondary">
			{{ formatMessage(messages.optionalNote) }}
		</p>
	</div>
</template>
