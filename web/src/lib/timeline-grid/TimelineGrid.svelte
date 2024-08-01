<script lang="ts">
  import Gallery from '@lib/swipey-gallery/Gallery.svelte';
  import type { ThumbnailBounds } from '@lib/swipey-gallery/thumbnail-bounds';
  import { slideForAsset, type SlideData } from '@lib/swipey-gallery/slide-data';
  import type { AssetId } from '@lib/apitypes';
  import type { ITimelineGrid } from '@lib/timeline-grid/timeline.svelte';
  import type { ActionReturn } from 'svelte/action';
  import GridTile from '@lib/ui/GridTile.svelte';
  import SegmentTitle from './SegmentTitle.svelte';
  import type { SelectState } from '@lib/ui/GridTile.svelte';
  import CreateGroupInput from './CreateGroupInput.svelte';

  type TimelineGridProps = {
    timeline: ITimelineGrid;
    scrollWrapper: HTMLElement;
  };

  let viewport = $state({ width: 0, height: 0 });
  let gallery: Gallery;

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

  function onAssetClick(assetIdx: number) {
    didMoveScrollToCurrentGalleryAsset = false;
    gallery.open(assetIdx);
  }

  function getThumbnailBounds(assetIndex: number): ThumbnailBounds {
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

  async function getSlide(assetIndex: number): Promise<SlideData | null> {
    const asset = await timeline.getOrLoadAssetAtIndex(assetIndex);
    if (asset === null) {
      return null;
    }
    await scrollToAssetIndex(assetIndex);
    return slideForAsset(asset);
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
            onAssetClick(item.assetIndex);
          }}
          onSelectToggled={() => {
            toggleAssetSelected(item.asset.id);
          }}
          selectState={getSelectState(item.asset.id)}
          bind:imgEl={thumbnailImgEls[item.assetIndex]}
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
  numSlides={timeline.totalNumAssets}
  {getThumbnailBounds}
  {getSlide}
  {scrollWrapper}
  {restoreScrollOnGalleryClose}
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
