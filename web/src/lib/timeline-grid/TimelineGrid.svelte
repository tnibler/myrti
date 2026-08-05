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
  import { SvelteMap } from 'svelte/reactivity';

  type TimelineGridProps = {
    timeline: ITimelineGrid;
    scrollWrapper: HTMLElement;
    openedAssetId: string | null;
  };

  let viewport = $state({ width: 0, height: 0 });
  let gallery: Gallery;

  let { timeline, scrollWrapper = $bindable(), openedAssetId }: TimelineGridProps = $props();
  let gridItemTransitionClass: string = $state('');
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
    const imgEl = document.getElementById(`thumb-${assetId}`);
    if (!imgEl || !(imgEl instanceof HTMLImageElement)) {
      return { rect: { x: 0, y: 0, width: 0, height: 0 } };
    }
    if (asset.repFile.rotationCorrection % 180 == 0) {
      return {
        rect: {
          x: imgEl.x,
          y: imgEl.y,
          width: imgEl.width,
          height: imgEl.height,
        },
      };
    } else {
      return {
        rect: {
          x: imgEl.x - (imgEl.height - imgEl.width) * 0.5,
          y: imgEl.y + (imgEl.height - imgEl.width) * 0.5,
          width: imgEl.height,
          height: imgEl.width,
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
  const visibleSections: (TimelineSection & { blocks: TimelineBlock[] })[] = $derived.by(() => {
    const ret: (TimelineSection & { blocks: TimelineBlock[] })[] = [];
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

  const sectionTops = $derived.by(() => {
    const ret = [];
    let top = 0;
    for (const section of timeline.sections) {
      ret.push(top);
      top += section.height;
    }
    return ret;
  });

  const imageEls: Map<string, HTMLImageElement> = new Map();
  const registerAction = (el: HTMLImageElement) => {
    imageEls.set(el.id, el);
    return () => {
      imageEls.delete(el.id);
    };
  };

  $inspect(timeline.sections);

  const previousRects = new Map<string, DOMRect>();
  $effect.pre(() => {
    timeline.sections;
    previousRects.clear();
    for (const [id, el] of imageEls) {
      const rect = el.getBoundingClientRect();
      previousRects.set(id, {
        top: rect.top,
        left: rect.left,
        width: rect.width,
        height: rect.height,
      });
    }
  });

  $effect(() => {
    timeline.sections;
    for (const [id, el] of imageEls) {
      const before = previousRects.get(id);
      if (!before) continue;
      const after = el.getBoundingClientRect();
      animateThumbMove(el, before, after);
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
    if (el.id === 'thumb-258') {
      console.log(el.id, before, after);
      console.log(el.getAnimations());
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
</script>

<div class="scroll-wrapper" bind:this={scrollWrapper} bind:clientHeight={viewport.height}>
  <section
    id="grid"
    bind:clientWidth={viewport.width}
    style:height={timeline.timelineHeight + 'px'}
  >
    <!-- {#each timeline.sections as section (section.sectionIdx)} -->
    <!-- {#if timeline.visibleSections.startIdx <= section.sectionIdx && section.sectionIdx < timeline.visibleSections.endIdx} -->
    <!-- {/if} -->
    <!-- {/each} -->
    <!-- eslint-disable-next-line svelte/require-each-key -->
    {#each visibleSections as section (section.sectionIdx)}
      <div
        class="w-full absolute contain-layout"
        style:top="{sectionTops[section.sectionIdx]}px"
        bind:clientHeight={
          null,
          (h) => {
            timeline.setActualSectionHeight(section.sectionIdx, h);
          }
        }
      >
        <!-- eslint-disable-next-line svelte/require-each-key -->
        {#each section.blocks as block}
          {@const blockHeight = Math.max(...block.gridItems.map((it) => it.top + it.height))}
          {#if block.blockType === 'default'}
            {@const segmentMargin = timeline.options.segmentMargin}
            {#if block.titleMajor !== null}
              <h2 class="text-2xl">
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
                <h3 class="text-lg">
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

          <div style:height="{blockHeight}px;" class="w-full relative">
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
      use:registerElementWithIntersectObserver
      id="section-{idx}"
      class="absolute w-full max-w-full invisible"
      style:top={sectionTops[idx] - timeline.options.loadWithinMargin + 'px'}
      style:height={section.height + timeline.options.loadWithinMargin * 2 + 'px'}
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
