<script lang="ts">
  import AddToAlbumDialog from '@lib/AddToAlbumDialog.svelte';
  import MainLayout from '@lib/MainLayout.svelte';
  import TimelineSelectAppBar from '@lib/TimelineSelectAppBar.svelte';
  import { api } from '@lib/apiclient';
  import {
    type TimelineOptions,
    type ITimelineGrid,
    createTimeline,
  } from '@lib/timeline-grid/timeline.svelte';
  import TimelineGrid from '@lib/timeline-grid/TimelineGrid.svelte';

  const layoutConfig: TimelineOptions = {
    targetRowHeight: 120,
    headerHeight: 50,
    segmentMargin: 20,
    boxSpacing: 4,
    loadWithinMargin: 300,
  };

  const timeline: ITimelineGrid = $state(createTimeline(layoutConfig, onAjustTimelineScrollY, api));
  const inSelectionMode = $derived(timeline.selectedAssets.size > 0);
  let timelineScrollWrapper: HTMLElement | null = $state(null);

  let addToAlbumDialog: AddToAlbumDialog | null = $state(null);

  function onAddToAlbumClicked() {
    addToAlbumDialog?.open();
  }

  function onAddToGroupClicked() {
    timeline.createGroupClicked();
  }

  async function onCreateAlbumSubmit(
    submitted: { action: 'createNew'; albumName: string } | { action: 'addTo'; albumId: string },
  ) {
    const assetIds = Array.from(timeline.selectedAssets.keys());
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

  function onAjustTimelineScrollY(params: {
    what: 'scrollTo' | 'scrollBy';
    scroll: number;
    ifScrollTopGt: number;
    behavior: 'smooth' | 'instant';
  }) {
    if (timelineScrollWrapper && timelineScrollWrapper.scrollTop > params.ifScrollTopGt) {
      if (params.what === 'scrollBy') {
        timelineScrollWrapper?.scrollBy({ top: params.scroll, behavior: params.behavior });
      } else if (params.what === 'scrollTo') {
        timelineScrollWrapper?.scrollTo({ top: params.scroll, behavior: params.behavior });
      }
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
    numAssetsSelected={timeline.selectedAssets.size}
    onCancelSelectClicked={() => timeline.clearSelection()}
    {onAddToAlbumClicked}
    {onAddToGroupClicked}
    onHideClicked={onHideAssetsClicked}
  />
{/snippet}
<AddToAlbumDialog bind:this={addToAlbumDialog} onSubmit={onCreateAlbumSubmit} />

<MainLayout
  content={timelineGrid}
  appBarOverride={timelineSelectAppBar}
  showAppBarOverride={inSelectionMode}
  activeSideBarEntry="timeline"
/>
