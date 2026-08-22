<script setup lang="ts">
import { CheckIcon, DownloadIcon, PlusIcon, SpinnerIcon } from '@modrinth/assets'
import { Avatar, Button, defineMessages, useVIntl } from '@modrinth/ui'
import { computed } from 'vue'

import type { CfMod } from '@/helpers/curseforge.ts'

const { formatMessage } = useVIntl()

const props = defineProps<{
	mod: CfMod
	installing?: boolean
	installed?: boolean
	canInstall?: boolean
}>()

const emit = defineEmits<{
	install: []
}>()

const messages = defineMessages({
	curseforgeBadge: {
		id: 'app.browse.curseforge.badge',
		defaultMessage: 'CurseForge',
	},
	distributionDisabledBadge: {
		id: 'app.browse.curseforge.distribution-disabled',
		defaultMessage: 'Not available via third-party tools',
	},
	installTooltipNoInstance: {
		id: 'app.browse.curseforge.install-tooltip.no-instance',
		defaultMessage: 'Open Explore from an instance to install CurseForge content',
	},
})

const allowDistribution = computed(() => props.mod.allowModDistribution !== false)
const canInstallMod = computed(() => allowDistribution.value && (props.canInstall ?? false))
const downloadCount = computed(() => Math.round(props.mod.downloadCount ?? 0))
</script>

<template>
	<div class="card-shadow bg-bg-raised rounded-xl overflow-clip flex flex-col gap-2 px-4 py-3">
		<div class="flex gap-2 items-center">
			<Avatar size="48px" :src="mod.logo?.url" />
			<div class="flex-1 min-w-0">
				<div class="flex items-center gap-2">
					<span class="font-bold text-contrast leading-normal line-clamp-1">{{ mod.name }}</span>
				</div>
				<div class="flex items-center gap-1 flex-wrap">
					<span
						class="text-xs font-semibold px-1.5 py-0.5 rounded-full bg-bg-orange text-orange"
					>
						{{ formatMessage(messages.curseforgeBadge) }}
					</span>
					<span
						v-if="!allowDistribution"
						class="text-xs font-semibold px-1.5 py-0.5 rounded-full bg-bg-red text-red"
					>
						{{ formatMessage(messages.distributionDisabledBadge) }}
					</span>
				</div>
			</div>
		</div>
		<p class="m-0 text-sm font-medium line-clamp-3 leading-tight h-[3.25rem]">
			{{ mod.summary }}
		</p>
		<div class="flex items-center justify-between gap-2 mt-auto">
			<div class="flex items-center gap-1 text-sm text-secondary font-semibold">
				<DownloadIcon />
				{{ downloadCount }}
			</div>
			<Button
				:disabled="!canInstallMod || installed || installing"
				:title="!allowDistribution ? undefined : !canInstall ? formatMessage(messages.installTooltipNoInstance) : undefined"
				color="brand"
				@click="emit('install')"
			>
				<SpinnerIcon v-if="installing" class="animate-spin" />
				<CheckIcon v-else-if="installed" />
				<PlusIcon v-else />
			</Button>
		</div>
	</div>
</template>
