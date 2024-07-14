<script lang="ts">
  import AddToAlbumDialog from '@lib/AddToAlbumDialog.svelte';
  import AddToGroupDialog from '@lib/AddToGroupDialog.svelte';
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
  let addToGroupDialog: AddToGroupDialog | null = $state(null);

  function onAddToAlbumClicked() {
    addToAlbumDialog?.open();
  }

  function onAddToGroupClicked() {
    timeline.createGroupClicked();
    return;
    // addToGroupDialog?.open();
  }

  async function onCreateGroupSubmit(submitted: { groupName: string }) {
    const assetIds = Object.keys(timeline.selectedAssets);
    await api
      .createTimelineGroup({
        assets: assetIds,
        name: submitted.groupName,
      })
      .catch((error) => {
        if (error.response) {
          console.log('error response');
          // Request made and server responded
          console.log(error.response.data);
          console.log(error.response.status);
          console.log(error.response.headers);
        } else if (error.request) {
          console.log('error request');
          // The request was made but no response was received
          console.log(error.request);
        } else {
          // Something happened in setting up the request that triggered an Error
          console.log('Error', error.message);
        }
      });
    addToGroupDialog?.close();
    timeline.clearSelection();
  }

  async function onCreateAlbumSubmit(
    submitted: { action: 'createNew'; albumName: string } | { action: 'addTo'; albumId: string },
  ) {
    const assetIds = Object.keys(timeline.selectedAssets);
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

  function onAjustTimelineScrollY(delta: number, ifScrollTopGt: number) {
    if (timelineScrollWrapper && timelineScrollWrapper.scrollTop > ifScrollTopGt) {
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
    numAssetsSelected={timeline.selectedAssets.size}
    onCancelSelectClicked={() => timeline.clearSelection()}
    {onAddToAlbumClicked}
    {onAddToGroupClicked}
    onHideClicked={onHideAssetsClicked}
  />
{/snippet}
<AddToAlbumDialog bind:this={addToAlbumDialog} onSubmit={onCreateAlbumSubmit} />
<AddToGroupDialog bind:this={addToGroupDialog} onSubmit={onCreateGroupSubmit} />

<MainLayout
  content={timelineGrid}
  appBarOverride={timelineSelectAppBar}
  showAppBarOverride={inSelectionMode}
  activeSideBarEntry="timeline"
/>
