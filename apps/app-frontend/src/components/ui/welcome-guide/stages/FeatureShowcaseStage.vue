<script setup lang="ts">
import { GameIcon, GaugeIcon, PaintbrushIcon, SearchIcon, ServerIcon } from '@modrinth/assets'
import { defineMessages, useVIntl } from '@modrinth/ui'
import { markRaw } from 'vue'

const { formatMessage } = useVIntl()

const messages = defineMessages({
	title: {
		id: 'app.welcome-guide.features.title',
		defaultMessage: 'What you can do',
	},
	contentTitle: {
		id: 'app.welcome-guide.features.content.title',
		defaultMessage: 'Search Modrinth and CurseForge',
	},
	contentDescription: {
		id: 'app.welcome-guide.features.content.description',
		defaultMessage: 'Find and install mods, modpacks, resource packs, and shaders from both sites.',
	},
	instancesTitle: {
		id: 'app.welcome-guide.features.instances.title',
		defaultMessage: 'Manage instances your way',
	},
	instancesDescription: {
		id: 'app.welcome-guide.features.instances.description',
		defaultMessage: 'Create and organize as many separate Minecraft setups as you need.',
	},
	serversTitle: {
		id: 'app.welcome-guide.features.servers.title',
		defaultMessage: 'Host servers in-app',
	},
	serversDescription: {
		id: 'app.welcome-guide.features.servers.description',
		defaultMessage: 'Spin up and manage Minecraft servers without leaving the launcher.',
	},
	performanceTitle: {
		id: 'app.welcome-guide.features.performance.title',
		defaultMessage: 'Tune performance',
	},
	performanceDescription: {
		id: 'app.welcome-guide.features.performance.description',
		defaultMessage: 'Control memory allocation, Java installations, and resource limits.',
	},
	customizeTitle: {
		id: 'app.welcome-guide.features.customize.title',
		defaultMessage: 'Make it yours',
	},
	customizeDescription: {
		id: 'app.welcome-guide.features.customize.description',
		defaultMessage: 'Switch themes and tweak the launcher to fit how you like to play.',
	},
})

const features = [
	{ icon: markRaw(SearchIcon), title: messages.contentTitle, description: messages.contentDescription },
	{ icon: markRaw(GameIcon), title: messages.instancesTitle, description: messages.instancesDescription },
	{ icon: markRaw(ServerIcon), title: messages.serversTitle, description: messages.serversDescription },
	{ icon: markRaw(GaugeIcon), title: messages.performanceTitle, description: messages.performanceDescription },
	{
		icon: markRaw(PaintbrushIcon),
		title: messages.customizeTitle,
		description: messages.customizeDescription,
	},
]
</script>

<template>
	<div class="flex flex-col gap-4">
		<h2 class="m-0 text-lg font-semibold text-contrast">
			{{ formatMessage(messages.title) }}
		</h2>
		<div class="flex flex-col gap-3">
			<div
				v-for="feature in features"
				:key="feature.title.id"
				class="flex items-start gap-3 rounded-xl border border-solid border-surface-5 bg-button-bg p-3"
			>
				<span
					class="flex size-9 shrink-0 items-center justify-center rounded-lg bg-brand-highlight text-brand"
				>
					<component :is="feature.icon" aria-hidden="true" class="size-5" />
				</span>
				<div class="flex flex-col gap-0.5">
					<h3 class="m-0 text-sm font-semibold leading-5 text-contrast">
						{{ formatMessage(feature.title) }}
					</h3>
					<p class="m-0 text-sm leading-5 text-secondary">
						{{ formatMessage(feature.description) }}
					</p>
				</div>
			</div>
		</div>
	</div>
</template>
