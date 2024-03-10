<script lang="ts">
	import AddToAlbumDialog from '$lib/AddToAlbumDialog.svelte';
	import MainLayout from '$lib/MainLayout.svelte';
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

	let addToAlbumDialog: AddToAlbumDialog | null = $state(null);

	function onAddToAlbumClicked() {
		addToAlbumDialog?.open();
	}

	async function onCreateAlbumSubmit({ albumName }: { albumName: string }) {
		// TODO this is terrible but the whole selection thing is going to change dw
		const assetIds = await Promise.all(
			Object.keys(timeline.selectedAssetIndices).map((idx) =>
				timeline.getAssetAtIndex(parseInt(idx)).then((a) => a.id)
			)
		);
		const response = await api.createAlbum({
			assets: assetIds,
			name: albumName,
			description: null
		});
		console.log(`albumId: ${response.albumId}`);
		addToAlbumDialog?.close();
		timeline.clearSelection();
	}
</script>

<AddToAlbumDialog bind:this={addToAlbumDialog} onSubmit={onCreateAlbumSubmit} />

{#snippet content()}
	<TimelineGrid {timeline} />
{/snippet}

{#snippet timelineSelectAppBar()}
	<TimelineSelectAppBar
		numAssetsSelected={Object.keys(timeline.selectedAssetIndices).length}
		onCancelSelectClicked={() => timeline.clearSelection()}
		{onAddToAlbumClicked}
	/>
{/snippet}

<MainLayout
	{content}
	appBarOverride={timelineSelectAppBar}
	showAppBarOverride={inSelectionMode}
	activeSideBarEntry="timeline"
/>
