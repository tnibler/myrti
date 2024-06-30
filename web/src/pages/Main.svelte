<script lang="ts">
  import AddToAlbumDialog from '@lib/AddToAlbumDialog.svelte';
  import MainLayout from '@lib/MainLayout.svelte';
  import TimelineSelectAppBar from '@lib/TimelineSelectAppBar.svelte';
  import { api } from '@lib/apiclient';
  import { type TimelineGridStore, createTimeline } from '@lib/store/timeline.svelte';
  import TimelineGrid from '@lib/timeline-grid/TimelineGrid.svelte';

  const layoutConfig = {
    targetRowHeight: 120,
    headerHeight: 50,
    sectionMargin: 20,
    segmentMargin: 20,
    boxSpacing: 4,
  };

  const timeline: TimelineGridStore = $state(
    createTimeline(layoutConfig, onAjustTimelineScrollY, api),
  );
  const inSelectionMode = $derived(Object.keys(timeline.selectedAssetIds).length > 0);
  let timelineScrollWrapper: HTMLElement | null = $state(null);

  let addToAlbumDialog: AddToAlbumDialog | null = $state(null);
  let addToGroupDialog: AddToAlbumDialog | null = $state(null);

  function onAddToAlbumClicked() {
    addToAlbumDialog?.open();
  }

  async function onCreateGroupSubmit(
    submitted: { action: 'createNew'; albumName: string } | { action: 'addTo'; albumId: string },
  ) {
    if (submitted.action === 'addTo') {
      console.error('wrong');
      return;
    }
    const assetIds = Object.keys(timeline.selectedAssetIds);
    await api.createAlbum({
      assets: assetIds,
      name: submitted.albumName,
      description: null,
    });
    addToGroupDialog?.close();
    timeline.clearSelection();
  }

  async function onCreateAlbumSubmit(
    submitted: { action: 'createNew'; albumName: string } | { action: 'addTo'; albumId: string },
  ) {
    const assetIds = Object.keys(timeline.selectedAssetIds);
    if (submitted.action === 'createNew') {
      await api.createAlbum({
        assets: assetIds,
        name: submitted.albumName,
        description: null,
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

  async function onHideAssetsClicked() {
    await timeline.hideSelectedAssets();
    timeline.clearSelection();
  }
</script>

{#snippet timelineGrid()}
  <TimelineGrid {timeline} bind:scrollWrapper={timelineScrollWrapper} />
{/snippet}

{#snippet timelineSelectAppBar()}
  <TimelineSelectAppBar
    numAssetsSelected={Object.keys(timeline.selectedAssetIds).length}
    onCancelSelectClicked={() => timeline.clearSelection()}
    {onAddToAlbumClicked}
    onHideClicked={onHideAssetsClicked}
  />
{/snippet}
<AddToAlbumDialog bind:this={addToAlbumDialog} onSubmit={onCreateAlbumSubmit} />
<AddToAlbumDialog bind:this={addToGroupDialog} onSubmit={onCreateGroupSubmit} />
<MainLayout
  content={timelineGrid}
  appBarOverride={timelineSelectAppBar}
  showAppBarOverride={inSelectionMode}
  activeSideBarEntry="timeline"
/>
