<script lang="ts">
  import Gallery from '@lib/swipey-gallery/Gallery.svelte';
  import type { ThumbnailBounds } from '@lib/swipey-gallery/thumbnail-bounds';
  import type { AssetId, AssetWithSpe } from '@api/myrti';
  import type { ITimelineGrid } from '@lib/timeline-grid/timeline.svelte';
  import type { ActionReturn } from 'svelte/action';
  import GridTile from '@lib/ui/GridTile.svelte';
  import SegmentTitle from './SegmentTitle.svelte';
  import type { SelectState } from '@lib/ui/GridTile.svelte';
  import CreateGroupInput from './CreateGroupInput.svelte';
  import type { PositionInTimeline, TimelineItem } from './timeline-types';
  import type { GallerySlide, SingleAssetSlide } from '@lib/swipey-gallery/gallery-types';

  type TimelineGridProps = {
    timeline: ITimelineGrid;
    scrollWrapper: HTMLElement;
  };

  let viewport = $state({ width: 0, height: 0 });
  let gallery: Gallery<PositionInTimeline>;

  let { timeline, scrollWrapper = $bindable() }: TimelineGridProps = $props();
  let thumbnailImgEls: Record<number, HTMLImageElement> = $state({});
  let gridItemTransitionClass: string | undefined = $state();
  let animationsDisabledToStart = true;
  let didMoveScrollToCurrentGalleryAsset = $state(false);
  let restoreScrollOnGalleryClose = $derived(!didMoveScrollToCurrentGalleryAsset);

  $effect(() => {
    setTimeout(() => {
      animationsDisabledToStart = false;
    }, 1000);
    timeline.setAnimationsEnabled = setGridItemAnimationEnabled;
  });

  $effect(() => {
    timeline.initialize(viewport);
  });

  // handle window resize (debounced)
  let resizeTimeout: number | null = null;
  $effect(() => {
    viewport.width;
    viewport.height;
    if (resizeTimeout != null) {
      clearTimeout(resizeTimeout);
      resizeTimeout = null;
    }
    resizeTimeout = setTimeout(() => {
      timeline.resize(viewport, scrollWrapper.scrollTop);
      resizeTimeout = null;
    }, 200);
  });

  $effect(() => {
    // null fields accumulate in thumbnailImgEls, so clear them periodically
    if (Object.keys(thumbnailImgEls).length > visibleItems.length * 5) {
      Object.keys(thumbnailImgEls)
        .map(parseInt)
        .filter((k) => thumbnailImgEls[k] === null)
        .forEach((k) => delete thumbnailImgEls[k]);
    }
  });

  const intersectionObserver = new IntersectionObserver(handleSectionIntersect, {
    // I don't know how rootMargin works; using scrollWrapper, its child <section> or document does not work correctly, so we just make the intersection test divs larger to achieve the same effect
    rootMargin: '0px',
  });

  export async function scrollToAssetIndex(index: number) {
    const marginTop = 100;
    const item = await timeline.moveViewToAsset(index);
    if (
      item !== null &&
      (item.top < scrollWrapper.scrollTop ||
        scrollWrapper.scrollTop + scrollWrapper.clientHeight <= item.top + item.height)
    ) {
      scrollWrapper.scrollTop = Math.max(0, item.top - marginTop);
      didMoveScrollToCurrentGalleryAsset = true;
    }
  }

  const disableGridItemAnimationDelayMs = 180 + 20;
  let disableGridItemAnimationTimeout: number | null = null;
  async function setGridItemAnimationEnabled(enabled: boolean) {
    if (animationsDisabledToStart) {
      return;
    }
    if (!enabled) {
      disableGridItemAnimationTimeout = setTimeout(() => {
        // gridItemTransitionClass = '';
      }, disableGridItemAnimationDelayMs);
    } else {
      if (disableGridItemAnimationTimeout) {
        clearTimeout(disableGridItemAnimationTimeout);
        disableGridItemAnimationTimeout = null;
      }
      await new Promise<void>((resolve) => {
        setTimeout(() => {
          gridItemTransitionClass = 'timeline-item-transition';
          resolve();
        }, 0);
      });
    }
  }

  function handleSectionIntersect(entries: IntersectionObserverEntry[]) {
    entries;
    timeline.onScrollChange(scrollWrapper.scrollTop);
  }

  function registerElementWithIntersectObserver(el: HTMLDivElement): ActionReturn {
    intersectionObserver.observe(el);
    return {
      destroy: () => {
        intersectionObserver.unobserve(el);
      },
    };
  }

  const visibleItems = $derived.by(() => {
    const items = timeline.items
      .slice(timeline.visibleItems.startIdx, timeline.visibleItems.endIdx)
      .map((item, index) => {
        return {
          ...item,
          /** index of Item in timeline before sorting */
          originalItemIndex: index,
        };
      });
    // sorted because keyed {#each} does not handle reordering items apparently
    items.sort((a, b) => a.key.localeCompare(b.key));
    return items;
  });

  function getSelectState(assetId: AssetId): SelectState {
    if (timeline.state === 'justLooking' && timeline.selectedAssets.size > 0) {
      const isSelected = timeline.selectedAssets.has(assetId);
      return { state: 'select', isSelected };
    } else if (timeline.state === 'justLooking') {
      return { state: 'default' };
    } else {
      return { state: 'unclickable' };
    }
  }

  function toggleAssetSelected(assetId: AssetId) {
    const isSelected = timeline.selectedAssets.has(assetId);
    timeline.setAssetSelected(assetId, !isSelected);
  }

  function onAssetClick(item: TimelineItem & ({ itemType: 'asset' } | { itemType: 'photoStack' })) {
    didMoveScrollToCurrentGalleryAsset = false;
    gallery.open(item.pos);
  }

  function getThumbnailBounds(pos: PositionInTimeline): ThumbnailBounds {
    return { rect: { x: 0, y: 0, width: 0, height: 0 } };
    const img = thumbnailImgEls[assetIndex];
    if (!img) {
      return { rect: { x: 0, y: 0, width: 0, height: 0 } };
    }
    return {
      rect: {
        x: img.x,
        y: img.y,
        width: img.width,
        height: img.height,
      },
    };
  }

  function slideForAsset(asset: AssetWithSpe): SingleAssetSlide {
    if (asset.assetType === 'image') {
      return {
        assetType: 'image',
        asset,
        size: { width: asset.width, height: asset.height },
        src: '/api/assets/original/' + asset.id,
        placeholderSrc: '/api/assets/thumbnail/' + asset.id + '/large/avif',
      };
    } else {
      const videoSource = asset.hasDash
        ? { videoSource: 'dash' as const, mpdManifestUrl: '/api/dash/' + asset.id + '/stream.mpd' }
        : {
            videoSource: 'original' as const,
            mimeType: asset.mimeType,
            src: '/api/assets/original/' + asset.id,
          };
      return {
        assetType: 'video',
        asset,
        size: { width: asset.width, height: asset.height },
        placeholderSrc: '/api/assets/thumbnail/' + asset.id + '/large/avif',
        ...videoSource,
      };
    }
  }

  async function getSlide(pos: PositionInTimeline): Promise<GallerySlide<PositionInTimeline>> {
    const item = await timeline.getItem(pos);
    if (item.itemType === 'asset') {
      const slide = slideForAsset(item);
      return { ...slide, pos, slideType: 'singleAsset' };
    } else {
      const coverSlide = slideForAsset(item.series.assets[item.coverIndex]);
      return {
        slideType: 'assetSeries',
        coverSlide,
        series: item.series,
        coverIndex: item.coverIndex,
        pos,
      };
    }
  }
</script>

<div class="scroll-wrapper" bind:this={scrollWrapper} bind:clientHeight={viewport.height}>
  <section
    id="grid"
    bind:clientWidth={viewport.width}
    style:height={timeline.timelineHeight + 'px'}
  >
    {#each timeline.sections as section, idx}
      <div
        use:registerElementWithIntersectObserver
        id="section-{idx}"
        class="absolute w-full max-w-full"
        style:top={section.top - timeline.options.loadWithinMargin + 'px'}
        style:height={section.height + timeline.options.loadWithinMargin * 2 + 'px'}
      ></div>
    {/each}
    {#each visibleItems as item (item.key)}
      {@const itemIndex = timeline.visibleItems.startIdx + item.originalItemIndex}
      {#if item.type === 'asset'}
        <GridTile
          className={gridItemTransitionClass}
          asset={item.asset}
          box={item}
          onAssetClick={() => {
            onAssetClick(item.timelineItem);
          }}
          onSelectToggled={() => {
            toggleAssetSelected(item.asset.id);
          }}
          selectState={getSelectState(item.asset.id)}
          bind:imgEl={thumbnailImgEls[item.assetIndex]}
        />
      {:else if item.type === 'photoStack'}
        <GridTile
          className={gridItemTransitionClass}
          asset={item.series.assets[item.coverIndex]}
          box={item}
          showStackIcon
          onAssetClick={() => {
            onAssetClick(item.timelineItem);
          }}
          onSelectToggled={() => {
            // rough and ugly
            const coverAssetId = item.series.assets[item.coverIndex].id;
            const isSelected = timeline.selectedAssets.has(coverAssetId);
            for (const asset of item.series.assets) {
              timeline.setAssetSelected(asset.id, !isSelected);
            }
          }}
          selectState={getSelectState(item.series.assets[item.coverIndex].id)}
          bind:imgEl={thumbnailImgEls[item.firstAssetIndex]}
        />
      {:else if item.type === 'segmentTitle'}
        <SegmentTitle
          className={gridItemTransitionClass}
          timelineItem={item}
          onHeightTooSmall={(height) => {
            timeline.setActualItemHeight(itemIndex, height);
          }}
        />
      {:else if item.type === 'createGroupTitleInput'}
        <CreateGroupInput
          {item}
          onSubmit={(title) => {
            timeline.confirmCreateGroup(title);
          }}
          onCancel={() => {
            timeline.cancelCreateGroup();
          }}
        />
      {/if}
    {/each}
    {#each timeline.addToGroupClickAreas as area}
      <div
        role="button"
        class="absolute cursor-pointer hover:bg-black/10 border-black/20 hover:border-black/40 border-2 rounded-lg"
        style="top: {area.top}px;  height: {area.height}px; left: 0px; width: 100%;"
        onclick={() => {
          timeline.addSelectedToExistingGroup(area.groupId);
        }}
      ></div>
    {/each}
  </section>
</div>

<Gallery
  bind:this={gallery}
  {getThumbnailBounds}
  {getSlide}
  {scrollWrapper}
  {restoreScrollOnGalleryClose}
  getNextSlidePosition={timeline.getNextItemPosition}
/>

<style>
  #grid {
    position: relative;
    contain: layout;
  }

  :global(.timeline-item-transition) {
    transition-property: top, left;
    transition-timing-function: ease-in-out;
    transition-duration: 180ms;
  }

  .scroll-wrapper {
    padding: 0px;
    height: 100%;
    width: 100%;
    max-width: 100%;
    position: relative;
    overflow-y: scroll;
  }
</style>
