<script setup lang="ts">
import { computed } from 'vue'

import { getTagMessage, SOURCE_TAGS, sortTagsForDisplay } from '../../../utils'
import { TagTagItem } from '../../base'
import TagsOverflow from '../TagsOverflow.vue'

// "modrinth"/"curseforge" are modeled as loader tags only for their
// formatted name/icon (see SOURCE_TAGS) -- excludeLoaders (forced on for
// resource packs/data packs, since those tabs don't want real loader tags
// like "vanilla"/"canvas" cluttering cards) must not sweep the source tag
// into the overflow "+N" bucket along with them.
function isLoader(tag: string) {
	return !SOURCE_TAGS.has(tag) && getTagMessage(tag, 'loader') !== undefined
}

function uniqueSorted(tags?: string[]) {
	return tags ? sortTagsForDisplay([...new Set(tags)]) : undefined
}

const props = withDefaults(
	defineProps<{
		tags: string[]
		extraTags?: string[]
		deprioritizedTags?: string[]
		excludeLoaders?: boolean
		maxTags?: number
	}>(),
	{
		maxTags: 5,
		excludeLoaders: false,
		extraTags: () => [],
		deprioritizedTags: () => [],
	},
)

const sortedTags = computed(() => uniqueSorted(props.tags))
const sortedExtraTags = computed(() => uniqueSorted(props.extraTags))
const filteredTags = computed(() => {
	if (!sortedTags.value) {
		return undefined
	}
	return sortedTags.value.filter(
		(tag) => !props.deprioritizedTags.includes(tag) && (!props.excludeLoaders || !isLoader(tag)),
	)
})

const visibleTags = computed(() => filteredTags.value?.slice(0, props.maxTags))
const overflowTags = computed(() => [
	...new Set([
		...(props.tags.filter((x) => !visibleTags.value?.includes(x)) ?? []),
		...(sortedExtraTags.value?.filter((x) => !visibleTags.value?.includes(x)) ?? []),
	]),
])
</script>

<template>
	<TagTagItem
		v-for="tag in visibleTags"
		:key="'visible-tag-' + tag"
		hide-non-loader-icon
		:tag="tag"
	/>
	<TagsOverflow
		v-if="overflowTags"
		:tags="overflowTags"
		class="smart-clickable:allow-pointer-events"
	/>
</template>
