<script lang="ts">
  import Gallery from '@lib/swipey-gallery/Gallery.svelte';
  import type { ThumbnailBounds } from '@lib/swipey-gallery/types';
  import { slideForAsset } from '@lib/swipey-gallery/asset-slide';
  import type { ITimelineGrid } from '@lib/timeline-grid/timeline.svelte';
  import type { ActionReturn } from 'svelte/action';
  import GridTile from '@lib/ui/GridTile.svelte';
  import SegmentTitle from './SegmentTitle.svelte';
  import type { SelectState } from '@lib/ui/GridTile.svelte';
  import CreateGroupInput from './CreateGroupInput.svelte';
  import type { OpenedSlide, PositionInTimeline, TimelineItem } from './timeline-types';
  import type { GallerySlide, GallerySlideData } from '@lib/swipey-gallery/gallery-types';
  import { tick } from 'svelte';

  type TimelineGridProps = {
    timeline: ITimelineGrid;
    scrollWrapper: HTMLElement;
  };

  let viewport = $state({ width: 0, height: 0 });
  let gallery: Gallery;

  let { timeline, scrollWrapper = $bindable() }: TimelineGridProps = $props();
  let gridItemTransitionClass: string | undefined = $state();
  let animationsDisabledToStart = true;
  let didMoveScrollToCurrentGalleryAsset = $state(false);
  let restoreScrollOnGalleryClose = $derived(!didMoveScrollToCurrentGalleryAsset);

  let currentSlide: GallerySlide<PositionInTimeline> | null = $state(null);
  const pagerSlides: {
    left: GallerySlide<PositionInTimeline> | null;
    current: GallerySlide<PositionInTimeline>;
    right: GallerySlide<PositionInTimeline> | null;
  } | null = $derived.by(() => {
    if (currentSlide === null) {
      return null;
    }
    const assetId =
      currentSlide.slideType === 'singleAsset'
        ? currentSlide.asset.id
        : currentSlide.series.assets[currentSlide.coverIndex].id;
    const currentPos = timeline.getItemForAsset(assetId).pos;
    const leftPos = timeline.getNextItemPosition(currentPos, 'left');
    const rightPos = timeline.getNextItemPosition(currentPos, 'right');
    timeline.items;
    return {
      left: leftPos !== null ? getSlide(timeline.getItemMustBeLoaded(leftPos)) : null,
      current: currentSlide,
      right: rightPos !== null ? getSlide(timeline.getItemMustBeLoaded(rightPos)) : null,
    };
  });
  $inspect(pagerSlides);

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

  const intersectionObserver = new IntersectionObserver(handleSectionIntersect, {
    // I don't know how rootMargin works; using scrollWrapper, its child <section> or document does not work correctly, so we just make the intersection test divs larger to achieve the same effect
    rootMargin: '0px',
  });

  export async function scrollToTimelineItem(pos: PositionInTimeline) {
    const marginTop = 100;
    const item = await timeline.getGridItemAtPosition(pos);
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

  function getSelectState(item: TimelineItem): SelectState {
    if (timeline.state === 'justLooking' && timeline.numAssetsSelected > 0) {
      const isSelected = timeline.isItemSelected(item);
      return { state: 'select', isSelected };
    } else if (timeline.state === 'justLooking') {
      return { state: 'default' };
    } else {
      return { state: 'unclickable' };
    }
  }

  function toggleItemSelected(item: TimelineItem) {
    const isSelected = timeline.isItemSelected(item);
    timeline.setItemSelected(item, !isSelected);
  }

  function onAssetClick(item: TimelineItem & ({ itemType: 'asset' } | { itemType: 'photoStack' })) {
    didMoveScrollToCurrentGalleryAsset = false;
    currentSlide = getSlide(timeline.getItemMustBeLoaded(item.pos));
    tick().then(() => {
      gallery.open();
    });
  }

  function onSlideNavigated(dir: 'left' | 'right') {
    if (currentSlide === null || pagerSlides === null) {
      throw new Error('what');
    }
    if (dir === 'left') {
      currentSlide = pagerSlides.left;
    } else {
      currentSlide = pagerSlides.right;
    }
  }

  function getThumbnailBounds(): ThumbnailBounds {
    const pos = currentSlide.pos;
    const imgEl = document.getElementById(
      `thumb${pos.sectionIndex}-${pos.segmentIndex}-${pos.itemIndex}`,
    );
    if (!imgEl || !(imgEl instanceof HTMLImageElement)) {
      return { rect: { x: 0, y: 0, width: 0, height: 0 } };
    }
    return {
      rect: {
        x: imgEl.x,
        y: imgEl.y,
        width: imgEl.width,
        height: imgEl.height,
      },
    };
  }

  function getSlide(item: TimelineItem): GallerySlide<PositionInTimeline> {
    // scrollToTimelineItem(item.pos);
    if (item.itemType === 'asset') {
      const slide = slideForAsset(item);
      return { ...slide, slideType: 'singleAsset', pos: item.pos };
    } else {
      const coverSlide = slideForAsset(item.series.assets[item.coverIndex]);
      return {
        slideType: 'assetSeries',
        coverSlide,
        series: item.series,
        coverIndex: item.coverIndex,
        pos: item.pos,
      };
    }
  }

  /** bound rectangles of grid items in timeline.addToGroupClickAreas */
  const clickAreaRects = $derived(
    timeline.addToGroupClickAreas.map((clickArea) => {
      let currentTop = Infinity;
      let currentBottom = -Infinity;
      let currentLeft = viewport.width;
      let currentRight = 0;
      for (const item of clickArea.gridItems) {
        currentTop = Math.min(item.top, currentTop);
        currentBottom = Math.max(item.top + item.height, currentBottom);
        if (
          item.type === 'asset' ||
          item.type === 'photoStack' ||
          (item.type === 'segmentTitle' && item.titleType === 'day')
        ) {
          currentLeft = Math.min(item.left, currentLeft);
          currentRight = Math.max(item.left + item.width, currentRight);
        }
      }
      return {
        groupId: clickArea.groupId,
        top: currentTop,
        left: currentLeft,
        width: currentRight - currentLeft,
        height: currentBottom - currentTop,
      };
    }),
  );
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
          showStackIcon={false}
          onAssetClick={() => {
            onAssetClick(item.timelineItem);
          }}
          onSelectToggled={() => {
            toggleItemSelected(item.timelineItem);
          }}
          imgElId={`thumb${item.timelineItem.pos.sectionIndex}-${item.timelineItem.pos.segmentIndex}-${item.timelineItem.pos.itemIndex}`}
          selectState={getSelectState(item.timelineItem)}
        />
      {:else if item.type === 'photoStack'}
        <GridTile
          className={gridItemTransitionClass}
          asset={item.series.assets[item.coverIndex]}
          box={item}
          showStackIcon={true}
          onAssetClick={() => {
            onAssetClick(item.timelineItem);
          }}
          onSelectToggled={() => {
            toggleItemSelected(item.timelineItem);
          }}
          imgElId={`thumb${item.timelineItem.pos.sectionIndex}-${item.timelineItem.pos.segmentIndex}-${item.timelineItem.pos.itemIndex}`}
          selectState={getSelectState(item.timelineItem)}
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
    {#each clickAreaRects as area}
      <button
        class="absolute z-20 hover:bg-black/10 border-black/20 hover:border-black/40 border-2 rounded-lg"
        style="top: {area.top}px;  height: {area.height}px; left: {area.left}px; width: {area.width}px;"
        onclick={() => {
          timeline.addSelectedToExistingGroup(area.groupId);
        }}
      ></button>
    {/each}
  </section>
</div>

{#if pagerSlides !== null}
  <Gallery
    bind:this={gallery}
    slides={pagerSlides}
    {onSlideNavigated}
    {getThumbnailBounds}
    {scrollWrapper}
    {restoreScrollOnGalleryClose}
  />
{/if}

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
