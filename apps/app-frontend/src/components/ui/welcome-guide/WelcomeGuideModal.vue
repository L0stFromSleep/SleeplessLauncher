<script setup lang="ts">
import { CheckIcon, RightArrowIcon } from '@modrinth/assets'
import { defineMessages, MultiStageModal, type StageConfigInput, useVIntl } from '@modrinth/ui'
import { markRaw, useTemplateRef } from 'vue'
import type { ComponentExposed } from 'vue-component-type-helpers'

import { get, set } from '@/helpers/settings.ts'

import AllSetStage from './stages/AllSetStage.vue'
import CurseForgeKeyStage from './stages/CurseForgeKeyStage.vue'
import FeatureShowcaseStage from './stages/FeatureShowcaseStage.vue'
import WelcomeIntroStage from './stages/WelcomeIntroStage.vue'

const { formatMessage } = useVIntl()

const messages = defineMessages({
	backButton: {
		id: 'app.welcome-guide.back-button',
		defaultMessage: 'Back',
	},
	nextButton: {
		id: 'app.welcome-guide.next-button',
		defaultMessage: 'Next',
	},
	getStartedButton: {
		id: 'app.welcome-guide.get-started-button',
		defaultMessage: 'Get started',
	},
	startPlayingButton: {
		id: 'app.welcome-guide.start-playing-button',
		defaultMessage: 'Start playing',
	},
	welcomeStageTitle: {
		id: 'app.welcome-guide.stages.welcome.title',
		defaultMessage: 'Welcome',
	},
	featuresStageTitle: {
		id: 'app.welcome-guide.stages.features.title',
		defaultMessage: 'What you can do',
	},
	curseforgeStageTitle: {
		id: 'app.welcome-guide.stages.curseforge.title',
		defaultMessage: 'Connect CurseForge',
	},
	doneStageTitle: {
		id: 'app.welcome-guide.stages.done.title',
		defaultMessage: "You're all set",
	},
})

const modal = useTemplateRef<ComponentExposed<typeof MultiStageModal>>('modal')

function nextStage() {
	modal.value?.nextStage()
}

function prevStage() {
	modal.value?.prevStage()
}

const stages: StageConfigInput<Record<string, never>>[] = [
	{
		id: 'welcome',
		title: formatMessage(messages.welcomeStageTitle),
		stageContent: markRaw(WelcomeIntroStage),
		nonProgressStage: true,
		leftButtonConfig: null,
		rightButtonConfig: {
			label: formatMessage(messages.getStartedButton),
			icon: markRaw(RightArrowIcon),
			iconPosition: 'after',
			color: 'brand',
			onClick: nextStage,
		},
		maxWidth: '520px',
	},
	{
		id: 'features',
		title: formatMessage(messages.featuresStageTitle),
		stageContent: markRaw(FeatureShowcaseStage),
		leftButtonConfig: { label: formatMessage(messages.backButton), onClick: prevStage },
		rightButtonConfig: { label: formatMessage(messages.nextButton), onClick: nextStage },
		maxWidth: '520px',
	},
	{
		id: 'curseforge',
		title: formatMessage(messages.curseforgeStageTitle),
		stageContent: markRaw(CurseForgeKeyStage),
		leftButtonConfig: { label: formatMessage(messages.backButton), onClick: prevStage },
		rightButtonConfig: { label: formatMessage(messages.nextButton), onClick: nextStage },
		maxWidth: '520px',
	},
	{
		id: 'done',
		title: formatMessage(messages.doneStageTitle),
		stageContent: markRaw(AllSetStage),
		leftButtonConfig: { label: formatMessage(messages.backButton), onClick: prevStage },
		rightButtonConfig: {
			label: formatMessage(messages.startPlayingButton),
			icon: markRaw(CheckIcon),
			color: 'brand',
			onClick: () => modal.value?.hide(),
		},
		maxWidth: '520px',
	},
]

async function markCompleted() {
	const settings = await get()
	if (settings.has_seen_welcome_guide) return
	settings.has_seen_welcome_guide = true
	await set(settings)
}

function handleHide() {
	void markCompleted()
}

function show() {
	modal.value?.setStage(0)
	modal.value?.show()
}

defineExpose({ show })
</script>

<template>
	<MultiStageModal ref="modal" :stages="stages" :context="{}" @hide="handleHide" />
</template>
