<script setup lang="ts">
import { defineMessages, useVIntl } from '@modrinth/ui'

import welcomeGuideLogo from '@/assets/branding/sleepless-launcher-logo.png'

const { formatMessage } = useVIntl()

const messages = defineMessages({
	title: {
		id: 'app.welcome-guide.intro.title',
		defaultMessage: 'Welcome to Sleepless Launcher',
	},
	description: {
		id: 'app.welcome-guide.intro.description',
		defaultMessage:
			"Let's take a minute to get you set up. We'll show you around and help you connect your own CurseForge API key so you can browse and install CurseForge content.",
	},
})
</script>

<template>
	<div class="flex flex-col items-center gap-6 py-4 text-center">
		<img :src="welcomeGuideLogo" alt="" class="size-20" />
		<div class="flex flex-col items-center gap-2">
			<h2 class="m-0 text-2xl font-semibold text-contrast">
				{{ formatMessage(messages.title) }}
			</h2>
			<p class="m-0 max-w-sm text-base leading-6 text-secondary">
				{{ formatMessage(messages.description) }}
			</p>
		</div>
	</div>
</template>
