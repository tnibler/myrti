<script lang="ts">
  import Gallery from '@lib/swipey-gallery/Gallery.svelte';
  import type { ThumbnailBounds } from '@lib/swipey-gallery/types';
  import type { ITimelineGrid } from '@lib/timeline-grid/timeline.svelte';
  import type { ActionReturn } from 'svelte/action';
  import GridTile from '@lib/ui/GridTile.svelte';
  import SegmentTitle from './SegmentTitle.svelte';
  import type { SelectState } from '@lib/ui/GridTile.svelte';
  import CreateGroupInput from './CreateGroupInput.svelte';
  import type {
    PositionInTimeline,
    TimelineBlock,
    TimelineItem,
    TimelineSection,
  } from './timeline-types';
  import type { SlideRef } from '@lib/swipey-gallery/gallery-types';
  import { path } from 'elegua';
  import { fade } from 'svelte/transition';

  type TimelineGridProps = {
    timeline: ITimelineGrid;
    scrollWrapper: HTMLElement;
    openedAssetId: string | null;
  };

  let viewport = $state({ width: 0, height: 0 });
  let gallery: Gallery;

  let { timeline, scrollWrapper = $bindable(), openedAssetId }: TimelineGridProps = $props();
  let gridItemTransitionClass: string = $state('');
  let gridAnimationsEnabled = false;
  let animationsDisabledToStart = true;
  let didMoveScrollToCurrentGalleryAsset = $state(false);
  let restoreScrollOnClose = $derived(!didMoveScrollToCurrentGalleryAsset);
  let useOpenTransition = $state(false);

  let currentSlide: SlideRef | null = $derived.by(() => {
    if (openedAssetId === null) {
      return null;
    }
    const item = timeline.getItemForAsset(openedAssetId);
    if (item === null) {
      return null;
    } else if (item.itemType === 'asset') {
      return { slideType: 'singleAsset', assetId: item.assetId };
    } else {
      return {
        slideType: 'assetSeries',
        assetSeriesId: item.seriesId,
        coverIndex: item.coverIndex,
      };
    }
  });
  let galleryOpen = $derived(openedAssetId !== null && currentSlide !== null);
  const pagerSlides: {
    left: SlideRef | null;
    current: SlideRef;
    right: SlideRef | null;
  } | null = $derived.by(() => {
    if (currentSlide === null) {
      return null;
    }
    const assetId =
      currentSlide.slideType === 'singleAsset'
        ? currentSlide.assetId
        : timeline.getAssetSeries(currentSlide.assetSeriesId).assetIds[currentSlide.coverIndex];
    const currentItem = timeline.getItemForAsset(assetId);
    const leftPos = timeline.getNextItemPosition(currentItem.pos, 'left');
    const rightPos = timeline.getNextItemPosition(currentItem.pos, 'right');
    return {
      left: leftPos !== null ? getSlideRef(timeline.getItemMustBeLoaded(leftPos)) : null,
      current: currentSlide,
      right: rightPos !== null ? getSlideRef(timeline.getItemMustBeLoaded(rightPos)) : null,
    };
  });

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
    if (!scrollWrapper) {
      return;
    }
    // eslint-disable-next-line @typescript-eslint/no-unused-expressions
    viewport.width;
    // eslint-disable-next-line @typescript-eslint/no-unused-expressions
    viewport.height;
    if (resizeTimeout != null) {
      clearTimeout(resizeTimeout);
      resizeTimeout = null;
    }
    resizeTimeout = setTimeout(() => {
      if (!scrollWrapper) {
        return;
      }
      timeline.resize(viewport, scrollWrapper.scrollTop);
      resizeTimeout = null;
    }, 200);
  });

  function handleSectionIntersect() {
    timeline.onSectionIntersectChanged(scrollWrapper.scrollTop);
  }

  const intersectionObserver = new IntersectionObserver(handleSectionIntersect, {
    // I don't know how rootMargin works; using scrollWrapper, its child <section> or document does not work correctly, so we just make the intersection test divs larger to achieve the same effect
    rootMargin: '0px',
  });
  function registerIntersectObserver(el: HTMLElement) {
    intersectionObserver.observe(el);
    return () => {
      intersectionObserver.unobserve(el);
    };
  }

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
        gridAnimationsEnabled = false;
      }, disableGridItemAnimationDelayMs);
    } else {
      if (disableGridItemAnimationTimeout) {
        clearTimeout(disableGridItemAnimationTimeout);
        disableGridItemAnimationTimeout = null;
      }
      await new Promise<void>((resolve) => {
        setTimeout(() => {
          gridAnimationsEnabled = true;
          resolve();
        }, 0);
      });
    }
  }

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
    useOpenTransition = true;
    if (item.itemType === 'asset') {
      $path = `/timeline/${item.assetId}`;
    } else {
      const series = timeline.getAssetSeries(item.seriesId);
      const id = series.assetIds[item.coverIndex];
      $path = `/timeline/${id}`;
    }
  }

  function onSlideNavigated(dir: 'left' | 'right') {
    if (currentSlide === null || pagerSlides === null) {
      throw new Error('what');
    }
    const toSlide = dir === 'left' ? pagerSlides.left : pagerSlides.right;
    if (!toSlide) {
      return;
    }

    if (toSlide.slideType === 'singleAsset') {
      $path = `/timeline/${toSlide.assetId}`;
    } else {
      const series = timeline.getAssetSeries(toSlide.assetSeriesId);
      const id = series.assetIds[toSlide.coverIndex];
      $path = `/timeline/${id}`;
    }
  }

  function getThumbnailBounds(sl: SlideRef | null): ThumbnailBounds {
    if (!sl) {
      return { rect: { x: 0, y: 0, width: 0, height: 0 } };
    }
    const assetId =
      sl.slideType === 'singleAsset'
        ? sl.assetId
        : timeline.getAssetSeries(sl.assetSeriesId).assetIds[sl.coverIndex];
    const currentItem = timeline.getItemForAsset(assetId);
    if (currentItem.itemType !== 'asset' && currentItem.itemType !== 'photoStack') {
      return { rect: { x: 0, y: 0, width: 0, height: 0 } };
    }
    const asset = timeline.getAsset(assetId);
    const img = document.getElementById(`thumb-${assetId}`)?.getBoundingClientRect();
    if (!img) {
      return { rect: { x: 0, y: 0, width: 0, height: 0 } };
    }
    if (asset.repFile.rotationCorrection % 180 == 0) {
      return {
        rect: {
          x: img.x,
          y: img.y,
          width: img.width,
          height: img.height,
        },
      };
    } else {
      return {
        rect: {
          x: img.x - (img.height - img.width) * 0.5,
          y: img.y + (img.height - img.width) * 0.5,
          width: img.height,
          height: img.width,
        },
      };
    }
  }

  function getSlideRef(item: TimelineItem): SlideRef {
    if (item.itemType === 'asset') {
      const asset = timeline.getAsset(item.assetId);
      return { slideType: 'singleAsset', assetId: asset.assetId };
    } else {
      return {
        slideType: 'assetSeries',
        assetSeriesId: item.seriesId,
        coverIndex: item.coverIndex,
      };
    }
  }

  /** bound rectangles of grid items in timeline.addToGroupClickAreas */
  const clickAreaRects = $derived(timeline.addToGroupClickAreas);
  const visibleSections: (TimelineSection & { blocks: TimelineBlock[]; sectionIdx: number })[] =
    $derived.by(() => {
      const ret: (TimelineSection & { blocks: TimelineBlock[]; sectionIdx: number })[] = [];
      for (
        let sectionIdx = timeline.visibleSections.startIdx;
        sectionIdx < timeline.visibleSections.endIdx;
        sectionIdx += 1
      ) {
        const section = timeline.sections[sectionIdx];
        if (section.segments !== null && section.blocks !== null) {
          ret.push({ sectionIdx, ...section });
        }
      }
      return ret;
    });

  const imageEls: Map<string, HTMLElement> = new Map();
  const registerAction = (el: HTMLElement) => {
    imageEls.set(el.id, el);
    return () => {
      imageEls.delete(el.id);
    };
  };

  const previousRects = new Map<string, DOMRect>();
  $effect.pre(() => {
    timeline.sections;
    previousRects.clear();
    for (const [id, el] of imageEls) {
      previousRects.set(id, el.getBoundingClientRect());
    }
  });

  $effect(() => {
    timeline.sections;
    if (gridAnimationsEnabled) {
      for (const [id, el] of imageEls) {
        const before = previousRects.get(id);
        if (!before) {
          continue;
        }
        const after = el.getBoundingClientRect();
        animateThumbMove(el, before, after);
      }
    }
  });

  function animateThumbMove(el: HTMLElement, before: DOMRect, after: DOMRect) {
    const dx = before.left - after.left;
    const dy = before.top - after.top;
    const sx = before.width / after.width;
    const sy = before.height / after.height;
    if (
      Math.abs(dx) < 0.5 &&
      Math.abs(dy) < 0.5 &&
      Math.abs(sx - 1) < 0.001 &&
      Math.abs(sy - 1) < 0.001
    ) {
      return;
    }
    const anim = el.animate(
      [
        {
          translate: `${dx}px ${dy}px`,
        },
        { translate: '0px' },
      ],
      { duration: 250, easing: 'ease' },
    );
    anim.onfinish = () => {
      // anim.commitStyles();
    };
  }

  let scrollTop = $state(0);
  function onScroll(e: UIEvent) {
    // console.log(scrollWrapper.scrollTop);
    timeline.onScrollChange(scrollWrapper.scrollTop);
    scrollTop = scrollWrapper.scrollTop;
  }

  const observer = new ResizeObserver((entries) => {
    const changedSections = new Map();
    for (const entry of entries) {
      const blockIdx = Number(entry.target.dataset.block);
      const sectionIdx = Number(entry.target.dataset.section);
      const height = entry.borderBoxSize?.[0]?.blockSize ?? entry.contentRect.height;
      if (height > 0) {
        changedSections.getOrInsert(sectionIdx, []).push([blockIdx, height]);
      }
    }
    for (const [sectionIdx, blockHeights] of changedSections.entries()) {
      timeline.setActualBlockHeight(sectionIdx, blockHeights);
    }
  });

  function registerBlockResize(node: HTMLElement) {
    observer.observe(node);
    return {
      destroy() {
        observer.unobserve(node);
      },
    };
  }
</script>

<div
  class="scroll-wrapper"
  bind:this={scrollWrapper}
  bind:clientHeight={viewport.height}
  onscroll={onScroll}
>
  <div
    role="scrollbar"
    aria-valuenow={timeline.scrollbarY}
    class="fixed h-screen w-16 bg-red-100 inset-e-3 z-1 select-none hover:cursor-row-resize"
  >
    {#each timeline.scrollbarMonths as month (`${month.year}-${month.month}`)}
      {#if month.showYear || month.showMonth}
        <div class="absolute w-full" style={`height: ${month.height}px; top: ${month.top}px;`}>
          {#if month.showYear}
            <div class="absolute h-full inset-e-6">
              {month.year}
            </div>
          {/if}
          {#if month.showMonth}
            <div class="absolute inset-e-2 rounded-full size-2 bg-gray-500"></div>
          {/if}
        </div>
      {/if}
    {/each}
    <div class="w-full absolute h-2 bg-red-400" style={`top: ${timeline.scrollbarY}px;`}></div>
  </div>

  <section
    id="grid"
    bind:clientWidth={viewport.width}
    style:height={timeline.timelineHeight + 'px'}
  >
    <!-- eslint-disable-next-line svelte/require-each-key -->
    {#each visibleSections as section (section.sectionIdx)}
      <div
        class="w-full absolute contain-layout"
        style:top="{timeline.sectionTops[section.sectionIdx]}px"
      >
        <!-- eslint-disable-next-line svelte/require-each-key -->
        {#each section.blocks as block, blockIdx (block.sortDate)}
          {@const blockTop = block.top + timeline.sectionTops[section.sectionIdx]}
          <div
            class="relative w-full"
            data-section={section.sectionIdx}
            data-block={blockIdx}
            {@attach registerBlockResize}
          >
            {#if block.blockType === 'default'}
              {@const segmentMargin = timeline.options.segmentMargin}
              {#if block.titleMajor !== null}
                <!-- NOTE: padding, not margins so clientHeight measures the correct thing -->
                <h2
                  {@attach registerAction}
                  class="text-3xl pt-3"
                  id={block.titleMajor.key}
                  in:fade
                >
                  {block.titleMajor.text}
                </h2>
              {/if}
              <div
                class="grid"
                style:grid-template-columns={block.titlesMinor
                  .map(
                    (title, idx) =>
                      `${title.width + (idx === block.titlesMinor.length - 1 ? 0 : segmentMargin)}fr`,
                  )
                  .join(' ')}
                style:width="{Math.max(...block.gridItems.map((it) => it.left + it.width))}px"
              >
                {#each block.titlesMinor as titleMinor}
                  <h3 {@attach registerAction} id={titleMinor.key} class="text-xl py-1" in:fade>
                    {titleMinor.text}
                  </h3>
                {/each}
              </div>
            {:else if block.blockType === 'createGroup'}
              <CreateGroupInput
                onSubmit={(title) => {
                  timeline.confirmCreateGroup(title);
                }}
                onCancel={() => timeline.cancelCreateGroup()}
              />
            {/if}

            <div style="height: {block.gridHeight}px;" class="w-full relative contain-strict">
              {#if blockTop < scrollTop + viewport.height + 1000 && scrollTop - 1000 < blockTop + block.fullHeight}
                <!-- {#if true} -->
                {#each block.gridItems as item (item.key)}
                  {#if item.type === 'asset'}
                    <GridTile
                      imgElAction={registerAction}
                      href="/timeline/{item.assetId}"
                      className={gridItemTransitionClass}
                      asset={timeline.getAsset(item.assetId)}
                      box={item}
                      showStackIcon={false}
                      onAssetClick={() => {
                        onAssetClick(item.timelineItem);
                      }}
                      onSelectToggled={() => {
                        toggleItemSelected(item.timelineItem);
                      }}
                      imgElId={`thumb-${item.assetId}`}
                      selectState={getSelectState(item.timelineItem)}
                    />
                  {:else if item.type === 'photoStack'}
                    {@const coverAsset = timeline.getAsset(
                      timeline.getAssetSeries(item.seriesId).assetIds[item.coverIndex],
                    )}
                    <GridTile
                      imgElAction={registerAction}
                      className={gridItemTransitionClass}
                      asset={coverAsset}
                      box={item}
                      showStackIcon={true}
                      onAssetClick={() => {
                        onAssetClick(item.timelineItem);
                      }}
                      onSelectToggled={() => {
                        toggleItemSelected(item.timelineItem);
                      }}
                      imgElId={`thumb-${coverAsset.assetId}`}
                      selectState={getSelectState(item.timelineItem)}
                    />
                  {/if}
                {/each}
              {/if}
            </div>
          </div>
        {/each}
      </div>
    {/each}
    {#each clickAreaRects as area (area.groupId)}
      <button
        class="absolute z-20 hover:bg-black/10 border-black/20 hover:border-black/40 border-2 rounded-lg"
        style="top: {area.top}px;  height: {area.height}px; left: {area.left}px; width: {area.width}px;"
        onclick={() => {
          timeline.addSelectedToExistingGroup(area.groupId);
        }}
      ></button>
    {/each}
  </section>
  {#each timeline.sections as section, idx}
    <div
      {@attach registerIntersectObserver}
      id="section-{idx}"
      class="absolute w-full max-w-full invisible"
      style:top={timeline.sectionTops[idx] - timeline.options.loadWithinMargin + 'px'}
      style:height={timeline.sectionHeights[idx] + timeline.options.loadWithinMargin * 2 + 'px'}
    ></div>
  {/each}
</div>

<Gallery
  bind:this={gallery}
  isOpen={galleryOpen}
  slides={pagerSlides}
  dataSource={timeline}
  {useOpenTransition}
  {onSlideNavigated}
  {getThumbnailBounds}
  onRotateClicked={() => {
    timeline.rotateAssetCW(currentSlide.assetId);
  }}
  onMirrorClicked={(axis) => {
    timeline.mirrorAsset(currentSlide.assetId, axis);
  }}
  {scrollWrapper}
  {restoreScrollOnClose}
  closeGallery={() => {
    $path = '/';
  }}
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
