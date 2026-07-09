<script lang="ts">
  import { untrack } from 'svelte';
  import type { Point, Size } from './util_types';
  import { type ZoomLevels, computeZoomLevels, computePanForChangedZoomLevel } from './zoom';
  import { clampPanToBounds, computePanBounds } from './pan-bounds';
  import type { ThumbnailBounds, SlideControls } from './types.ts';
  import { fade } from 'svelte/transition';
  import SlideImage from './SlideImage.svelte';
  import SlideVideo from './SlideVideo.svelte';
  import './slide.css';
  import type { GalleryDataSource, SingleAssetSlide, SlideRef } from './gallery-types';
  import { mdiStar } from '@mdi/js';
  import { slideForAsset } from './asset-slide';
  import { getGalleryContext, type GalleryContext } from './context';
  import * as R from 'remeda';
  import { goto } from 'elegua';

  export type OpenTransitionParams = {
    fromBounds: ThumbnailBounds;
    onTransitionEnd: () => void;
  };

  export type SlideProps = {
    slide: SlideRef;
    isActive: boolean;
    openTransition: OpenTransitionParams | null;
    showContent: boolean;
    showUi: boolean;
    onContentReady: (() => void) | undefined;
    dataSource: GalleryDataSource;
    viewportSize: Size;
  };
  let {
    slide,
    isActive,
    openTransition,
    showContent,
    showUi,
    onContentReady,
    dataSource,
    viewportSize,
  }: SlideProps = $props();

  const galleryContext: GalleryContext = getGalleryContext();

  let pan: Point = $state({ x: 0, y: 0 });
  let panAreaSize: Size = $derived(viewportSize); // TODO missing padding like photoswipe
  const centerX = $derived(panAreaSize.width / 2);
  const centerY = $derived(panAreaSize.height / 2);

  let selectedSeriesIndex: number | null = $state(
    slide.slideType === 'singleAsset' ? null : slide.coverIndex,
  );
  let savedSlide: SlideRef | null = null;
  $effect(() => {
    if (!R.isDeepEqual(savedSlide, slide)) {
      savedSlide = slide;
    }
    selectedSeriesIndex = slide.slideType === 'singleAsset' ? null : slide.coverIndex;
  });

  const slideToDisplay = $derived.by(() => {
    if (slide.slideType === 'singleAsset') {
      const asset = dataSource.getAsset(slide.assetId);
      return { slideType: slide.slideType, ...slideForAsset(asset) };
    } else {
      const series = dataSource.getAssetSeries(slide.assetSeriesId);
      const indexInSeries = selectedSeriesIndex ?? slide.coverIndex;
      const asset = dataSource.getAsset(series.assetIds[indexInSeries]);
      return { slideType: slide.slideType, series, indexInSeries, ...slideForAsset(asset) };
    }
  });
  let savedSlideToDisplay: typeof slideToDisplay | null = null;

  let zoomLevels: ZoomLevels = $derived(
    computeZoomLevels({
      maxSize: slideToDisplay.size,
      panAreaSize,
    }),
  );
  /** Image DOM element has size imgSize * domZoom */
  let domZoom: number = $state(1);
  /** zoom applied as CSS scale on top of domZoom. Only temporary during zoom motions, 
	after that zoom gets applied to the DOM element and transform scale is reset. */
  let cssTransformZoom: number = $state(1);
  const effectiveZoom = $derived(domZoom * cssTransformZoom);
  const panBounds = $derived(computePanBounds(slideToDisplay.size, panAreaSize, effectiveZoom));
  let slideImage: SlideImage | null = $state(null);
  let slideVideo: SlideVideo | null = $state(null);
  let placeholderEl: HTMLImageElement | null = $state(null);
  let cursor: null | 'zoom-in' | 'grab' | 'grabbing' = $derived.by(() => {
    if (effectiveZoom <= zoomLevels.fit) {
      return 'zoom-in';
    }
    if (isGrabbing) {
      return 'grabbing';
    }
    return 'grab';
  });

  type PlaceholderTransition = 'No' | 'Running' | 'Finished';

  let placeholderTransitionState = $state('No' as PlaceholderTransition);
  let contentHasLoaded = $state(false);
  let isContentVisible = $state(false);
  let placeholderVisible = $derived(!isContentVisible || placeholderTransitionState === 'Running');
  /** Wait this long after the real content is ready to hide the placeholder to reveal the <img> underneath.
	Without this, there is a flicker on some devices/browsers. */
  const PLACEHOLDER_HIDE_DELAY = $derived(slideToDisplay.assetType === 'image' ? 0 : 0);
  let zoomWrapperDiv: HTMLDivElement | null = $state(null);

  // for some reason the slideImage/slideVideo bindings don't get unset when the bound component
  // is removed, so we do it manually here
  $effect(() => {
    slideToDisplay.assetType;
    untrack(() => {
      slideImage = slideToDisplay.assetType === 'image' ? slideImage : null;
      slideVideo = slideToDisplay.assetType === 'video' ? slideVideo : null;
    });
  });

  $effect(() => {
    // small delay between image being loaded and allowed to be shown and actually doing it
    // for flicker reasons. Not perfect but pretty good
    if (!isContentVisible && contentHasLoaded && placeholderTransitionState !== 'Running') {
      isContentVisible = true;
    }
  });

  $effect(() => {
    // handle viewport size changes and zoom slide to at least fill viewport
    if (effectiveZoom < zoomLevels.min) {
      // slide is smaller than viewport
      cssTransformZoom = 1;
      domZoom = zoomLevels.min;
      pan = panBounds.center;
    } else if (effectiveZoom > zoomLevels.min && !userHasZoomed) {
      // user has not zoomed, slide should fit viewport
      cssTransformZoom = 1;
      domZoom = zoomLevels.fit;
      pan = panBounds.center;
    }
  });

  let userHasZoomed = $state(false);
  let lastZoomPoint: Point | null = $state(null);
  export const controls: SlideControls = {
    get canBePanned() {
      return effectiveZoom > zoomLevels.fit;
    },
    get panBounds() {
      return panBounds;
    },
    get currentZoomLevel() {
      return cssTransformZoom * domZoom;
    },
    set pan(value) {
      pan = value;
    },
    get pan() {
      return pan;
    },
    get zoomLevels() {
      return zoomLevels;
    },
    get canBeZoomed() {
      return true;
    },
    get size() {
      return slideToDisplay.size;
    },
    setZoomLevel: (newZoom) => {
      cssTransformZoom = newZoom / domZoom;
      userHasZoomed = cssTransformZoom > zoomLevels.fit;
    },
    applyCurrentZoomPan: () => {
      domZoom *= cssTransformZoom;
      cssTransformZoom = 1;
    },
    toggleZoom: (p: Point) => {
      if (userHasZoomed) {
        animateZoomTo('reset');
        lastZoomPoint = null;
      } else {
        animateZoomTo({ toPoint: p, toLevel: zoomLevels.secondary });
        lastZoomPoint = p;
      }
    },
    zoomIn: () => {
      if (userHasZoomed && lastZoomPoint) {
        animateZoomTo({
          toPoint: lastZoomPoint,
          toLevel: effectiveZoom * 2,
        });
      } else {
        const point = { x: innerWidth / 2, y: innerHeight / 2 };
        animateZoomTo({
          toPoint: point,
          toLevel: zoomLevels.secondary,
        });
        lastZoomPoint = point;
      }
    },
    zoomOut: () => {
      animateZoomTo('reset');
    },
    get isAtMaxZoom() {
      return effectiveZoom >= zoomLevels.max;
    },
    get isAtMinZoom() {
      return effectiveZoom <= zoomLevels.fit;
    },
    closeTransition,
    onGrabbingStateChange: (grabbing: boolean) => {
      isGrabbing = grabbing;
    },
  };

  let isGrabbing = $state(false);
  let transitionTransformClass = $state(false);
  let { width, height } = $derived({
    width: slideToDisplay.size.width * domZoom,
    height: slideToDisplay.size.height * domZoom,
  });

  $effect(() => {
    if (!isActive && slideToDisplay) {
      untrack(() => {
        initializeForNewSlide(slideToDisplay, panAreaSize);
      });
    }
  });

  $effect(() => {
    if (!slideToDisplay) {
      return;
    }
    if (savedSlideToDisplay && R.isDeepEqual(slideToDisplay.size, savedSlideToDisplay.size)) {
      return;
    }
    savedSlideToDisplay = slideToDisplay;
    // reinitialize zoom/pan transition states everytime slide is displayed
    untrack(() => {
      initializeForNewSlide(slideToDisplay, panAreaSize);
    });
  });

  $effect(() => {
    if (openTransition != null && placeholderEl && placeholderTransitionState === 'No') {
      addOpenTransition(placeholderEl, openTransition);
    }
  });

  function initializeForNewSlide(slide: SingleAssetSlide, panAreaSize: Size) {
    const newZoomLevels = computeZoomLevels({
      maxSize: untrack(() => slide.size),
      panAreaSize: untrack(() => panAreaSize),
    });
    const newPanBounds = computePanBounds(slide.size, panAreaSize, newZoomLevels.fit);
    domZoom = newZoomLevels.fit;
    cssTransformZoom = 1;
    pan = {
      x: newPanBounds.center.x,
      y: newPanBounds.center.y,
    };
  }

  function addOpenTransition(el: HTMLImageElement, t: OpenTransitionParams) {
    const transform = getTransformToFitThumbnail(t.fromBounds);
    el.style.transform = transform;
    placeholderTransitionState = 'Running';

    requestAnimationFrame(() => {
      const listener = (e: TransitionEvent) => {
        if (e.target === el) {
          el.removeEventListener('transitionend', listener, false);
          el.removeEventListener('transitioncancel', listener, false);
          placeholderTransitionState = 'Finished';
          t.onTransitionEnd();
        }
      };
      el.addEventListener('transitionend', listener, false);
      el.addEventListener('transitioncancel', listener, false);

      // Photoswipe opener.js:249 does werid async waiting for decoding/timeout stuff
      // saying that something doesn't work on firefox.
      // Can't reproduce, this works afaict.
      requestAnimationFrame(() => {
        el.style.transform = '';
      });
    });
  }

  function getTransformToFitThumbnail(bounds: ThumbnailBounds): string {
    if (bounds.crop) {
      console.warn('TODO open anim with cropped bounds not implemented');
    }
    const scaleX = bounds.rect.width / width;
    const scaleY = bounds.rect.height / height;
    const translateY =
      -viewportSize.height / 2 +
      bounds.rect.height / 2 +
      bounds.rect.y +
      panBounds.center.y -
      pan.y;

    const translateX =
      -viewportSize.width / 2 +
      bounds.rect.width / 2 +
      bounds.rect.x +
      (panBounds.center.x - pan.x);

    return `translate3d(${translateX}px, ${translateY}px, 0) scale3d(${scaleX}, ${scaleY}, 1)`;
  }

  function closeTransition(toBounds: ThumbnailBounds, onTransitionEnd: () => void) {
    const transform = getTransformToFitThumbnail(toBounds);
    // apply transition to placeholder if the content element hasn't loaded yet
    if (placeholderEl) {
      // The placeholder may still be visible while the content element is already present
      // because of the delay between the image content being ready and the placeholder being hidden.
      // If we close in this intermediary state, the content element should be hidden before animating
      // the placeholder.
      isContentVisible = false;

      const listener = (e: TransitionEvent) => {
        if (e.target === placeholderEl) {
          placeholderEl?.removeEventListener('transitionend', listener, false);
          placeholderEl?.removeEventListener('transitioncancel', listener, false);
          onTransitionEnd();
        }
      };
      placeholderEl.addEventListener('transitionend', listener, false);
      placeholderEl.addEventListener('transitioncancel', listener, false);
      placeholderTransitionState = 'Running';

      requestAnimationFrame(() => {
        if (!placeholderEl) {
          return;
        }
        placeholderEl.style.transform = transform;
      });
    } else if (slideImage) {
      slideImage.closeTransition(transform, onTransitionEnd);
    } else if (slideVideo) {
      slideVideo.closeTransition(transform, onTransitionEnd);
    } else {
      // catch all so gallery closes no matter what weird in-between states we get into
      onTransitionEnd();
    }
  }

  function onSlideContentReady() {
    if (onContentReady) {
      onContentReady();
    }
    contentHasLoaded = true; // hide thumbnail and show real content
  }

  function animateZoomTo(p: { toLevel: number; toPoint: Point } | 'reset') {
    if (!zoomWrapperDiv) {
      return;
    }
    const endListener = () => {
      transitionTransformClass = false;
      domZoom *= cssTransformZoom;
      cssTransformZoom = 1;
      zoomWrapperDiv?.removeEventListener('transitionend', endListener, false);
      zoomWrapperDiv?.removeEventListener('transitioncancel', endListener, false);
    };
    const startListener = () => {
      transitionTransformClass = false;
      zoomWrapperDiv?.removeEventListener('transitionstart', startListener, false);
    };
    zoomWrapperDiv.addEventListener('transitionstart', startListener, false);
    zoomWrapperDiv.addEventListener('transitionend', endListener, false);
    zoomWrapperDiv.addEventListener('transitioncancel', endListener, false);
    transitionTransformClass = true;
    if (p === 'reset') {
      const currentZoom = domZoom * cssTransformZoom;
      cssTransformZoom *= zoomLevels.fit / currentZoom;
      pan = panBounds.center;
      userHasZoomed = false;
    } else {
      const currentZoom = domZoom * cssTransformZoom;
      // toPoint is with origin at top left, pan here is with origin in the center
      const point = {
        x: p.toPoint.x - centerX,
        y: p.toPoint.y - centerY,
      };
      const newPan = {
        x: computePanForChangedZoomLevel('x', p.toLevel, currentZoom, point, point, pan),
        y: computePanForChangedZoomLevel('y', p.toLevel, currentZoom, point, point, pan),
      };
      cssTransformZoom = p.toLevel / domZoom;
      pan = clampPanToBounds(newPan, panBounds);
      userHasZoomed = true;
    }
  }

  async function onSeriesSelectionChanged(assetId: string, isSeriesSelection: boolean) {
    await galleryContext.setAssetSeriesSelection(assetId, isSeriesSelection);
  }
</script>

<div
  bind:this={zoomWrapperDiv}
  class="zoom-wrapper"
  class:transition-transform={transitionTransformClass}
  class:cursor-zoom-in={cursor === 'zoom-in'}
  class:cursor-grab={cursor === 'grab'}
  class:cursor-grabbing={cursor === 'grabbing'}
  style="
  	transform-origin: 0px 0px 0px;
	transform: translate3d({pan.x + centerX}px, {pan.y +
    centerY}px, 0) scale3d({cssTransformZoom}, {cssTransformZoom}, 1);"
>
  {#key `${slideToDisplay.asset.id}, ${slideToDisplay.size.width}, ${slideToDisplay.size.height}`}
    {#if slideToDisplay.assetType === 'image' && showContent}
      <SlideImage
        bind:this={slideImage}
        size={{ width, height }}
        slideData={slideToDisplay}
        isVisible={isContentVisible}
        onContentReady={onSlideContentReady}
      />
    {:else if slideToDisplay.assetType === 'video' && showContent}
      <SlideVideo
        bind:this={slideVideo}
        size={{ width, height }}
        slideData={slideToDisplay}
        isVisible={isContentVisible}
        {isActive}
        onContentReady={onSlideContentReady}
      />
    {/if}
  {/key}
  {#if placeholderVisible}
    <!-- svelte-ignore a11y-missing-attribute -->
    <img
      class="placeholder max-w-none"
      bind:this={placeholderEl}
      out:fade={{ duration: 100, delay: PLACEHOLDER_HIDE_DELAY }}
      src={slideToDisplay.slideType === 'singleAsset'
        ? slideToDisplay.placeholderSrc
        : slideToDisplay.placeholderSrc}
      style:width="{width}px"
      style:height="{height}px"
      style:user-select="none"
      class:slide-transition-transform={placeholderTransitionState === 'Running'}
    />
  {/if}
</div>
{#if slideToDisplay.slideType === 'assetSeries' && showUi}
  <div class="absolute bottom-4 left-1/2 -translate-x-1/2 max-w-full md:max-w-7xl md:h-48 h-24">
    <div
      class="rounded-xl bg-black/70 backdrop-blur px-1 md:px-2 py-2 md:py-4 flex flex-row overflow-x-auto h-full overflow-y-hidden"
    >
      {#each slideToDisplay.series.assetIds as assetId, indexInSeries (assetId)}
        {@const asset = dataSource.getAsset(assetId)}
        {@const isSelection = slideToDisplay.series.selectionIndices.indexOf(indexInSeries) !== -1}
        <div class="group relative h-full shrink-0 mx-1 md:mx-3">
          <div class="absolute top-1 right-1">
            <input
              type="checkbox"
              class="sr-only"
              id={`seriesCheck${assetId}`}
              checked={isSelection}
              onchange={(e) => {
                e.stopImmediatePropagation();
                e.stopPropagation();
                onSeriesSelectionChanged(assetId, e.target.checked);
              }}
            />
            <label for={`seriesCheck${assetId}`}>
              <svg
                onpointerdowncapture={(e) => e.stopPropagation()}
                onpointerupcapture={(e) => e.stopPropagation()}
                width="24"
                height="24"
                viewBox="0 0 24 24"
                class="rounded-md bg-black/70 backdrop-blur p-1
                        opacity-0 hover:fill-white group-has-checked:fill-white group-has-checked:opacity-100 group-hover:opacity-100 group-has-focus:opacity-100"
              >
                <path d={mdiStar} />
              </svg>
            </label>
          </div>
          <a
            href="javascript:;"
            onclick={(e) => {
              e.stopPropagation();
              e.preventDefault();
              isContentVisible = false;
              contentHasLoaded = false;
              goto('/timeline/' + asset.id);
              selectedSeriesIndex = indexInSeries;
            }}
            onpointerup={(e) => e.stopPropagation()}
            onpointerdown={(e) => e.stopPropagation()}
          >
            <img
              src="/api/assets/thumbnail/{asset.id}/small/avif"
              class={'bg-black transition-transform max-h-full max-w-full object-cover rounded-sm ' +
                (selectedSeriesIndex === indexInSeries ? 'border-solid border-3 border-white' : '')}
              style:max-width="none"
              style:rotate={asset.rotationCorrection ? asset.rotationCorrection + 'deg' : null}
            />
          </a>
        </div>
      {/each}
    </div>
  </div>
{/if}

<style>
  .zoom-wrapper {
    position: absolute;
  }

  .transition-transform {
    transition: transform;
    transition-duration: 150ms;
  }

  .placeholder {
    position: absolute;
  }
</style>
