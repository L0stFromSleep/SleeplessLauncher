<script setup lang="ts">
import { defineMessages, injectNotificationManager, StyledInput, useVIntl } from '@modrinth/ui'
import { ref, watch } from 'vue'

import { get, set } from '@/helpers/settings.ts'

const { handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()

const messages = defineMessages({
	apiKeyTitle: {
		id: 'app.settings.curseforge.api-key.title',
		defaultMessage: 'CurseForge API key',
	},
	apiKeyPlaceholder: {
		id: 'app.settings.curseforge.api-key.placeholder',
		defaultMessage: 'Paste your personal CurseForge API key...',
	},
	apiKeyDescription: {
		id: 'app.settings.curseforge.api-key.description',
		defaultMessage:
			'Required to search and install CurseForge content. Get your own key from the CurseForge for Studios developer console (console.curseforge.com). Stored locally on this device only.',
	},
})

const fetchSettings = await get()
const settings = ref(fetchSettings)

watch(
	settings,
	async () => {
		const setSettings = JSON.parse(JSON.stringify(settings.value))

		if (!setSettings.curseforge_api_key) {
			setSettings.curseforge_api_key = null
		}

		await set(setSettings).catch(handleError)
	},
	{ deep: true },
)
</script>

<template>
	<div>
		<div class="flex flex-col gap-2.5">
			<h2 class="m-0 text-lg font-semibold text-contrast">
				{{ formatMessage(messages.apiKeyTitle) }}
			</h2>
			<StyledInput
				id="curseforge-api-key"
				v-model="settings.curseforge_api_key"
				autocomplete="off"
				type="password"
				:placeholder="formatMessage(messages.apiKeyPlaceholder)"
				wrapper-class="w-full"
			/>
			<p class="m-0 leading-tight">
				{{ formatMessage(messages.apiKeyDescription) }}
			</p>
		</div>
	</div>
</template>
