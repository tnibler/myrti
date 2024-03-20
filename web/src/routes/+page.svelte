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

	const timeline: TimelineGridStore = $state(
		createTimeline(layoutConfig, onAjustTimelineScrollY, api)
	);
	const inSelectionMode = $derived(Object.keys(timeline.selectedAssetIds).length > 0);
	let timelineScrollWrapper: HTMLElement | null = $state(null);

	let addToAlbumDialog: AddToAlbumDialog | null = $state(null);

	function onAddToAlbumClicked() {
		addToAlbumDialog?.open();
	}

	async function onCreateAlbumSubmit(
		submitted: { action: 'createNew'; albumName: string } | { action: 'addTo'; albumId: string }
	) {
		const assetIds = Object.keys(timeline.selectedAssetIds);
		if (submitted.action === 'createNew') {
			await api.createAlbum({
				assets: assetIds,
				name: submitted.albumName,
				description: null
			});
		} else if (submitted.action === 'addTo') {
			await api.appendAssetsToAlbum({ assetIds }, { params: { id: submitted.albumId } });
		}
		addToAlbumDialog?.close();
		timeline.clearSelection();
	}

	function onAjustTimelineScrollY(delta: number, minApplicableTop: number) {
		if (timelineScrollWrapper && timelineScrollWrapper.scrollTop > minApplicableTop) {
			timelineScrollWrapper?.scrollBy(0, delta);
		}
	}
</script>

<AddToAlbumDialog bind:this={addToAlbumDialog} onSubmit={onCreateAlbumSubmit} />

{#snippet content()}
	<TimelineGrid {timeline} bind:bodyWrapper={timelineScrollWrapper} />
{/snippet}

{#snippet timelineSelectAppBar()}
	<TimelineSelectAppBar
		numAssetsSelected={Object.keys(timeline.selectedAssetIds).length}
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
