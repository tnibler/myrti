<script lang="ts">
	import TimelineSelectAppBar from '$lib/TimelineSelectAppBar.svelte';
	import { api } from '$lib/apiclient';
	import AppBar from '$lib/appbar/AppBar.svelte';
	import { type TimelineGridStore, createTimeline } from '$lib/store/timeline.svelte';
	import TimelineGrid from '$lib/timeline-grid/TimelineGrid.svelte';
	import Sidebar from './Sidebar.svelte';

	const layoutConfig = {
		targetRowHeight: 120,
		headerHeight: 50,
		sectionMargin: 20,
		segmentMargin: 20,
		boxSpacing: 4
	};

	const timeline: TimelineGridStore = $state(createTimeline(layoutConfig, api));
	const inSelectionMode = $derived(Object.keys(timeline.selectedAssetIndices).length > 0);
</script>

<div class="flex flex-col h-dvh">
	{#if !inSelectionMode}
		<AppBar />
	{:else}
		<TimelineSelectAppBar
			numAssetsSelected={Object.keys(timeline.selectedAssetIndices).length}
			onCancelSelectClicked={() => timeline.clearSelection()}
		/>
	{/if}
	<div id="page" class="flex-1">
		<Sidebar />
		<div id="content">
			<TimelineGrid {timeline} />
		</div>
	</div>
</div>

<style>
	#page {
		overflow-y: hidden;
		display: flex;
	}

	#content {
		flex-grow: 1;
	}

	@media screen and (max-width: 600px) {
		#sidebar {
			display: none;
		}
	}
</style>
