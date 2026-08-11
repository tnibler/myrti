<script lang="ts">
  import AddToAlbumDialog from '@lib/AddToAlbumDialog.svelte';
  import MainLayout from '@lib/MainLayout.svelte';
  import TimelineSelectAppBar from '@lib/TimelineSelectAppBar.svelte';
  import {
    type TimelineOptions,
    type ITimelineGrid,
    createTimeline,
  } from '@lib/timeline-grid/timeline.svelte';
  import TimelineGrid from '@lib/timeline-grid/TimelineGrid.svelte';
  import { appendAssetsToAlbum, createAlbum } from '@api/myrti';
  import { setGalleryContext, type GalleryContext } from '@lib/swipey-gallery/context';
  import { MediaQuery } from 'svelte/reactivity';

  const smallScreen = new MediaQuery('max-width: 639px');
  const layoutConfig: TimelineOptions = $derived(
    smallScreen.current
      ? {
          targetRowHeight: 100,
          headerHeight: 35,
          segmentMargin: 8,
          boxSpacing: 2,
          loadWithinMargin: 300,
        }
      : {
          targetRowHeight: 160,
          headerHeight: 50,
          segmentMargin: 16,
          boxSpacing: 4,
          loadWithinMargin: 500,
        },
  );

  const { openedAssetId } = $props();

  const timeline: ITimelineGrid = $state(createTimeline(layoutConfig));
  const inSelectionMode = $derived(timeline.numAssetsSelected > 0);
  let timelineScrollWrapper: HTMLElement | null = $state(null);

  const galleryContext: GalleryContext = $state({
    setAssetHidden: async (assetId) => {},
    setAssetSeriesSelection: async (assetId, isSeriesSelection) => {
      await timeline.setAssetSeriesSelection(assetId, isSeriesSelection);
    },
  });
  setGalleryContext(galleryContext);

  let addToAlbumDialog: AddToAlbumDialog | null = $state(null);

  function onAddToAlbumClicked() {
    addToAlbumDialog?.open();
  }

  function onAddToGroupClicked() {
    timeline.createGroupClicked();
  }

  function onCreateStackClicked() {
    timeline.createStackClicked();
  }

  async function onCreateAlbumSubmit(
    submitted: { action: 'createNew'; albumName: string } | { action: 'addTo'; albumId: string },
  ) {
    const assetIds = Array.from(timeline.selectedAssetIds);
    if (submitted.action === 'createNew') {
      await createAlbum({
        assets: assetIds,
        name: submitted.albumName,
        description: null,
      });
    } else if (submitted.action === 'addTo') {
      await appendAssetsToAlbum(submitted.albumId, { assetIds });
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

  const numAssetsSelected: number = $derived(timeline.numAssetsSelected);
</script>

{#snippet timelineGrid()}
  <div class="h-screen flex flex-col">
    <div class="w-full flex-none h-32"></div>
    <div class="relative w-full flex-1 min-h-1">
      <TimelineGrid {timeline} {openedAssetId} bind:scrollWrapper={timelineScrollWrapper} />
    </div>
  </div>
{/snippet}

{#snippet timelineSelectAppBar()}
  <TimelineSelectAppBar
    {numAssetsSelected}
    cancelSelect={{ visible: numAssetsSelected > 0, onClick: () => timeline.clearSelection() }}
    addToAlbum={{ visible: true, onClick: onAddToAlbumClicked }}
    addToGroup={{ visible: timeline.editGroupEnabled === 'add', onClick: onAddToGroupClicked }}
    removeFromGroup={{
      visible: timeline.editGroupEnabled === 'remove',
      onClick: () => {
        timeline.removeFromGroupClicked();
      },
    }}
    createStack={{ visible: timeline.createStackEnabled, onClick: onCreateStackClicked }}
    deleteStack={{ visible: timeline.deleteStackEnabled, onClick: () => {} }}
    addToStack={{ visible: timeline.addToStackEnabled, onClick: () => {} }}
    hide={{ visible: true, onClick: onHideAssetsClicked }}
  />
{/snippet}
<AddToAlbumDialog bind:this={addToAlbumDialog} onSubmit={onCreateAlbumSubmit} />

<MainLayout
  content={timelineGrid}
  appBarOverride={timelineSelectAppBar}
  showAppBarOverride={inSelectionMode}
  activeSideBarEntry="timeline"
/>
