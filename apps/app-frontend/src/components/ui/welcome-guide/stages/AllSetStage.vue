<script setup lang="ts">
import { CheckCircleIcon } from '@modrinth/assets'
import { defineMessages, useVIntl } from '@modrinth/ui'

const { formatMessage } = useVIntl()

const messages = defineMessages({
	title: {
		id: 'app.welcome-guide.done.title',
		defaultMessage: "You're all set",
	},
	description: {
		id: 'app.welcome-guide.done.description',
		defaultMessage:
			'You can revisit this anytime, and manage your CurseForge key from Settings → Integrations → CurseForge. Have fun!',
	},
})
</script>

<template>
	<div class="flex flex-col items-center gap-4 py-4 text-center">
		<CheckCircleIcon aria-hidden="true" class="size-12 text-brand" />
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
