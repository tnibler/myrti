<script lang="ts" module>
  import type { GalleryDataSource, SlideRef } from './gallery-types';
  export type PagerProps = {
    topOffset: number;

    slides: {
      left: SlideRef | null;
      current: SlideRef;
      right: SlideRef | null;
    } | null;
    onSlideNavigated: (dir: 'left' | 'right') => void;
    dataSource: GalleryDataSource;
    useOpenTransition: boolean;

    getThumbnailBounds: () => ThumbnailBounds;
    closeGallery: () => void;
    onOpenTransitionFinished: () => void;
    onRotateClicked: () => void;
    onMirrorClicked: (axis: 'horizontal' | 'vertical') => void;
  };

  export type GalleryControls = {
    currentSlide: SlideControls | null;
    pager: PagerControls;
    animations: AnimationControls;
    close: () => void;
    onVerticalDrag: (ratio: number) => void;
  };

  export type PagerState = {
    readonly viewportSize: { width: number; height: number };
    readonly isShifted: boolean;
    readonly currentSlideX: number;
    readonly x: number;
  };

  export type PagerControls = PagerState & {
    moveSlideAnimate: (to: 'left' | 'right' | 'backToCenter') => void;
    moveXBy: (delta: number) => void;
    close: () => void;
  };
</script>

<script lang="ts">
  import { onMount } from 'svelte';
  import { newGestureController } from './gestures';
  import { newAnimationControls, type AnimationControls } from './animations';
  import type { ThumbnailBounds, SlideControls } from './types.ts';
  import type { OpenTransitionParams } from './Slide.svelte';
  import { fade } from 'svelte/transition';
  import * as R from 'remeda';
  import InfoPanel from './InfoPanel.svelte';
  import Slide from './Slide.svelte';
  import {
    EyeOff,
    FlipHorizontal2,
    FlipVertical2,
    Info,
    RotateCw,
    X,
    ZoomIn,
    ZoomOut,
  } from '@lucide/svelte';
  import { intl } from '@lib/i18next';

  let {
    slides,
    onSlideNavigated,
    getThumbnailBounds,
    closeGallery,
    dataSource,
    useOpenTransition,
    onOpenTransitionFinished,
    topOffset,
    onRotateClicked,
    onMirrorClicked,
  }: PagerProps = $props();

  let viewport = $state({ width: 0, height: 0 });
  const slideSpacing = 0.1;
  const slideWidth = $derived(viewport.width + viewport.width * slideSpacing);

  /** used for transform/offset calculations */
  let currentShift = $state(0);
  let isSidePanelOpen: boolean = $state(false);

  type SlideHolderState = {
    id: number;
    slide: SlideRef | null;
    openTransition: OpenTransitionParams | null;
    isActive: boolean;
    showContent: boolean;
    isContentReady: boolean;
  };
  // permutation of the SlideHolders that get shuffled while scrolling
  // holderOrder[0] is the index into holderStates for the SlideHolder to the left of the screen,
  // [1] the currently visible one and [2] the one off to the right
  let holderOrder = $state([0, 1, 2]);
  let holderStates: SlideHolderState[] = $state(
    (() => {
      // holderOrder is the identity mapping at the beginning, so id == index initially for the SlideHolders
      return [0, 1, 2].map((id) => {
        const sl = slides[['left' as const, 'current' as const, 'right' as const][id]];
        const openTransition =
          id === 1 && useOpenTransition
            ? {
                onTransitionEnd: afterOpenTransition,
                fromBounds: getThumbnailBounds(sl),
              }
            : null;
        return {
          // maybe hide left and right holders until open anim finished? see main-scroll.js:111
          id: id,
          slide: sl,
          openTransition,
          isActive: id === 1,
          showContent: id === 1,
          isContentReady: false,
        };
      });
    })(),
  );
  const canMoveLeft = $derived(holderStates[holderOrder[0]].slide !== null);
  const canMoveRight = $derived(holderStates[holderOrder[2]].slide !== null);
  const xTransformSlideCenter = $derived(-currentShift * slideWidth * (1 + slideSpacing));
  let xTransformOffset = $state(0);
  let xTransform = $derived(xTransformSlideCenter + xTransformOffset);
  const transformString = $derived(`translate3d(${Math.round(xTransform)}px, 0px, 0px)`);
  let backgroundOpacity = $state(0);
  /** enable CSS transition when assigning backgroundOpacity. Only set on open and close. */
  let backgroundOpacityTransition = $state(true);

  let hasMouse = $state(false);

  const animations: AnimationControls = newAnimationControls();
  const slideComponents: Slide[] = $state([]);
  const slide = $derived(slideComponents[holderOrder[1]]?.controls);
  const hideUiTimeoutDuration = 60000;
  let hideUiTimeout: ReturnType<typeof setTimeout> | null = setTimeout(
    onHideUiTimeout,
    hideUiTimeoutDuration,
  );
  let savedSlide: SlideRef | null = null;
  $effect(() => {
    if (slides?.current && !R.isDeepEqual(slides.current, savedSlide)) {
      savedSlide = slides.current;
    }
  });

  const pagerControls: PagerControls = {
    get viewportSize() {
      return viewport;
    },
    get currentSlideX() {
      return xTransformSlideCenter;
    },
    get isShifted() {
      return xTransformOffset !== 0;
    },
    get x() {
      return xTransform;
    },
    moveXBy: (delta) => {
      const SWIPE_END_FRICTION = 0.3;
      const hittingLeftWall = 0 < delta && holderStates[holderOrder[0]].slide === null;
      const hittingRightWall = delta < 0 && holderStates[holderOrder[2]].slide === null;
      if (hittingLeftWall || hittingRightWall) {
        xTransformOffset += delta * SWIPE_END_FRICTION;
      } else {
        xTransformOffset += delta;
      }
    },
    moveSlideAnimate,
    close,
  };
  const gallery: GalleryControls = {
    get currentSlide() {
      return slide;
    },
    get pager() {
      return pagerControls;
    },
    get animations() {
      return animations;
    },
    close: () => {
      closeGallery();
    },
    onVerticalDrag: (ratio) => {
      if (ratio > 0.2) {
        backgroundOpacity = 0;
      } else {
        backgroundOpacity = 1;
      }
      // if (Math.abs(backgroundOpacity - 1 + ratio) > 0.02) {
      //   backgroundOpacity = 1 - ratio;
      // }
    },
  };
  const gestureController = newGestureController(gallery, () => {
    hasMouse = true;
  });

  let pagerWrapper: HTMLElement;

  onMount(() => {
    backgroundOpacity = 1;
    bindEvents();
    return () => {
      unbindEvents();
    };
  });

  function afterOpenTransition() {
    backgroundOpacityTransition = false;
    onOpenTransitionFinished();
  }

  function bindEvents() {
    pagerWrapper.onpointerdown = (e) => {
      if (!uiVisible) {
        showUi();
        // don't initiate drag or anything if ui was hidden
        return;
      }
    };
    pagerWrapper.onpointercancel = gestureController.onPointerUp;
    document.documentElement.onpointerleave = gestureController.onPointerUp;
    pagerWrapper.onmousemove = () => {
      showUi();
    };
  }

  function unbindEvents() {
    pagerWrapper.onpointerdown = null;
    document.documentElement.onpointerleave = null;
    pagerWrapper.onpointercancel = null;
    pagerWrapper.onclick = null;
    pagerWrapper.onmousemove = null;
  }

  function onHideUiTimeout() {
    uiVisible = false;
  }

  function showUi() {
    uiVisible = true;
    if (hideUiTimeout) {
      clearTimeout(hideUiTimeout);
      hideUiTimeout = null;
    }
    hideUiTimeout = setTimeout(onHideUiTimeout, hideUiTimeoutDuration);
  }

  function moveSlideAnimate(direction: 'left' | 'right' | 'backToCenter') {
    const offLimitsLeft = direction === 'left' && !canMoveLeft;
    const offLimitsRight = direction === 'right' && !canMoveRight;
    if (offLimitsLeft || offLimitsRight) {
      direction = 'backToCenter';
    }
    let shiftDiff = 0;
    if (direction === 'left') {
      shiftDiff = -1;
    } else if (direction === 'right') {
      shiftDiff = 1;
    }
    const index = currentShift + shiftDiff;
    if (direction !== 'backToCenter') {
      holderStates[holderOrder[1]].isActive = false;
    }
    const destX = -index * slideWidth * (1 + slideSpacing);
    animations.stopAnimationsFor('pager');
    animations.startSpringAnimation(
      {
        start: xTransformOffset,
        end: destX - xTransformSlideCenter,
        velocity: 0,
        frequency: 30,
        dampingRatio: 1, //0.7,
        onUpdate: (x: number) => {
          xTransformOffset = x;
        },
        onFinish: () => {
          xTransformOffset = 0;
          if (direction !== 'backToCenter') {
            currentShift = index;
            reorderSlideHoldersAfterAnim(direction);
          }
        },
      },
      'pager',
    );
  }

  /** @returns false if there is no more slide to move to, true otherwise */
  export function moveSlide(direction: 'left' | 'right'): boolean {
    const offLimitsLeft = direction === 'left' && !canMoveLeft;
    const offLimitsRight = direction === 'right' && !canMoveRight;
    if (offLimitsLeft || offLimitsRight) {
      return false;
    }
    animations.stopAllAnimations();
    holderStates[holderOrder[1]].isActive = false;
    currentShift += direction === 'left' ? -1 : 1;
    xTransformOffset = 0;
    reorderSlideHoldersAfterAnim(direction);
    return true;
  }

  function reorderSlideHoldersAfterAnim(didShift: 'left' | 'right') {
    animations.stopAnimationsFor('pan');
    const previousActiveHolder: SlideHolderState = holderStates[holderOrder[1]];
    let movedHolder: SlideHolderState;
    // TODO Photoswipe resets transforms here if currentShift >= 50
    if (didShift === 'right') {
      holderOrder = [holderOrder[1], holderOrder[2], holderOrder[0]];
      movedHolder = holderStates[holderOrder[2]];
    } else {
      holderOrder = [holderOrder[2], holderOrder[0], holderOrder[1]];
      movedHolder = holderStates[holderOrder[0]];
    }
    const newActiveHolder = holderStates[holderOrder[1]];
    previousActiveHolder.isActive = false;
    newActiveHolder.isActive = true;
    newActiveHolder.showContent = true;
    // not setting previousActiveHolder.showContent = false, because it's not getting assigned a new slide
    // if the current slide is already loaded, movedHolder can start loading slide content right away.
    movedHolder.showContent = newActiveHolder.isContentReady;
    console.assert(
      newActiveHolder.slide !== null,
      'newActiveHolder.slidePosition is null after shuffling SlideHolders',
    );
    onSlideNavigated(didShift);
  }

  // TODO: handle changes in series while keeping current index in series
  $effect(() => {
    if (!slides) {
      return;
    }
    const leftHolder = holderStates[holderOrder[0]];
    if (!R.isDeepEqual(leftHolder.slide, slides.left)) {
      leftHolder.slide = slides.left;
      leftHolder.openTransition = null;
      leftHolder.isContentReady = false;
    }
  });
  $effect(() => {
    if (!slides) {
      return;
    }
    const currentHolder = holderStates[holderOrder[1]];
    if (!R.isDeepEqual(currentHolder.slide, slides.current)) {
      currentHolder.slide = slides.current;
      currentHolder.openTransition = null;
      currentHolder.isContentReady = false;
    }
  });
  $effect(() => {
    if (!slides) {
      return;
    }
    const rightHolder = holderStates[holderOrder[2]];
    if (!R.isDeepEqual(rightHolder.slide, slides.right)) {
      rightHolder.slide = slides.right;
      rightHolder.openTransition = null;
      rightHolder.isContentReady = false;
    }
  });

  export async function close() {
    uiVisible = false;
    const thumbnailBounds = getThumbnailBounds(savedSlide);
    backgroundOpacityTransition = true;
    backgroundOpacity = 0;
    return new Promise<void>((resolve) => {
      if (slide) {
        slide.closeTransition(thumbnailBounds, () => {
          resolve();
        });
      } else {
        resolve();
      }
    });
  }

  function onSlideContentReady(slideHolderId: number) {
    holderStates[holderOrder[slideHolderId]].isContentReady = true;
    // if the currently shown slide is ready, start loading those to the left and right
    if (slideHolderId == holderOrder[1]) {
      holderStates[holderOrder[0]].showContent = true;
      holderStates[holderOrder[2]].showContent = true;
    }
  }

  let isZoomOutDisabled = $derived(slide != null && slide.isAtMinZoom);
  let isZoomInDisabled = $derived(slide != null && slide.isAtMaxZoom);
  function onZoomInClicked() {
    if (slide && slide.canBeZoomed) {
      slide.zoomIn();
    }
  }

  function onZoomOutClicked() {
    if (slide && slide.canBeZoomed) {
      slide.zoomOut();
    }
  }

  let uiVisible = $state(true);
</script>

<div
  class="
  fixed top-0 left-0 w-full h-dvh
  flex flex-row
  touch-none overflow-hidden z-20"
  style:cursor={uiVisible ? 'default' : 'none'}
  style:top={`${topOffset}px`}
>
  <!--I think offsetWidth/Height is correct? slides seem drawn over the scrollbar -->
  <div
    class="grow relative z-[100]"
    bind:this={pagerWrapper}
    bind:offsetHeight={viewport.height}
    bind:offsetWidth={viewport.width}
  >
    <div
      class="w-full h-full top-0 left-0 bg-black z-0 transition-opacity duration-300 ease-in-out"
      style:opacity={backgroundOpacity}
      style:will-change="opacity"
      class:transition-opacity={backgroundOpacityTransition}
    ></div>
    {#if viewport.height > 0.0 && viewport.width > 0.0}
      <div class="absolute top-0 left-0 w-full h-full" style="transform: {transformString};">
        {#each holderStates as slideHolder (slideHolder.id)}
          <!-- currentShift - 1 because there is still one slideHolder to the left of the viewport when currentShift is 0 -->
          {@const x =
            (currentShift - 1 + holderOrder.indexOf(slideHolder.id)) *
            (1 + slideSpacing) *
            slideWidth}
          <div
            id="id-{slideHolder.id}"
            class="slide-holder"
            style="transform: translate3d({Math.round(x)}px, 0px, 0px);"
            onpointerdown={(e) => {
              gestureController.onPointerDown(e);
            }}
            onclick={(e) => {
              gestureController.onClick(e);
            }}
            onpointerup={(e) => {
              gestureController.onPointerUp(e);
            }}
            onpointermove={(e) => gestureController.onPointerMove(e)}
          >
            {#if slideHolder.slide !== null}
              {#key [slideHolder.slide.assetId, slideHolder.slide.assetSeriesId, slideHolder.slide.coverIndex]}
                <Slide
                  bind:this={slideComponents[slideHolder.id]}
                  isActive={slideHolder.isActive}
                  viewportSize={viewport}
                  {dataSource}
                  openTransition={slideHolder.openTransition}
                  showContent={slideHolder.showContent}
                  showUi={uiVisible}
                  slide={slideHolder.slide}
                  onContentReady={() => onSlideContentReady(slideHolder.id)}
                />
              {/key}
            {/if}
          </div>
        {/each}
      </div>
    {/if}
    {#if uiVisible}
      <div
        class="absolute top-0 left-0 w-full h-full flex flex-col z-10 pointer-events-none"
        out:fade
        onpointerdown={(e) => {
          e.stopPropagation();
        }}
        onpointerup={(e) => {
          e.stopPropagation();
        }}
      >
        <div
          class="flex flex-row flex-none justify-end items-center
    h-16 px-2 gap-4 bg-gradient-to-b from-black/50 pointer-events-auto"
        >
          <button
            class="p-2"
            class:button-visible={hasMouse}
            title={intl('rotate_cw')}
            onclick={() => {
              onRotateClicked();
            }}
          >
            <RotateCw color="white" />
          </button>
          <button
            class="p-2"
            class:button-visible={hasMouse}
            title={intl('flip_horizontal')}
            onclick={() => {
              onMirrorClicked('horizontal');
            }}
          >
            <FlipHorizontal2 color="white" />
          </button>
          <button
            class="p-2"
            class:button-visible={hasMouse}
            title={intl('flip_vertical')}
            onclick={() => {
              onMirrorClicked('vertical');
            }}
          >
            <FlipVertical2 color="white" />
          </button>
          <button
            class="p-2"
            title={intl('zoom_out')}
            class:button-visible={hasMouse}
            onclick={() => onZoomOutClicked()}
            disabled={isZoomOutDisabled}
          >
            <ZoomOut color={isZoomOutDisabled ? '#aaa' : 'white'} />
          </button>
          <button
            class="p-2"
            title={intl('zoom_in')}
            class:button-visible={hasMouse}
            onclick={() => onZoomInClicked()}
            disabled={isZoomInDisabled}
          >
            <ZoomIn color={isZoomInDisabled ? '#aaa' : 'white'} />
          </button>
          <button
            class="p-2"
            class:button-visible={hasMouse}
            title={intl('hide')}
            onclick={() => {}}
          >
            <EyeOff color="white" />
          </button>
          <button
            class="p-2"
            class:button-visible={hasMouse}
            title={intl('file_details')}
            onclick={() => {
              isSidePanelOpen = !isSidePanelOpen;
              moveSlideAnimate('backToCenter');
            }}
          >
            <Info color="white" />
          </button>
          <button
            class="p-4"
            title={intl('close_gallery')}
            class:button-visible={hasMouse}
            onclick={() => closeGallery()}
          >
            <X color="white" />
          </button>
        </div>
        <div class="flex flex-row flex-1 items-center justify-between {hasMouse ? '' : 'hidden'} ">
          <button
            class="pl-5 pointer-events-auto h-96"
            onclick={() => moveSlide('left')}
            disabled={!canMoveLeft}
          >
            <svg
              class={canMoveLeft ? 'fill-white' : 'fill-white/30'}
              viewBox="0 0 60 60"
              width="60"
              height="60"><path d="M29 43l-3 3-16-16 16-16 3 3-13 13 13 13z"></path></svg
            >
          </button>
          <button
            class="pr-5 pointer-events-auto h-96"
            onclick={() => moveSlide('right')}
            disabled={!canMoveRight}
          >
            <svg
              class="{canMoveRight ? 'fill-white' : 'fill-white/30'} -scale-x-[1]"
              viewBox="0 0 60 60"
              width="60"
              height="60"><path d="M29 43l-3 3-16-16 16-16 3 3-13 13 13 13z"></path></svg
            >
          </button>
        </div>
      </div>
    {/if}
  </div>

  <div class={'bg-white z-120 transition-all w-96 ' + (isSidePanelOpen ? 'mr-0' : 'mr-[-24rem]')}>
    {#if slides?.current}
      {@const slide = slides.current}
      <InfoPanel
        asset={slide.slideType === 'singleAsset'
          ? dataSource.getAsset(slide.assetId)
          : dataSource.getAsset(
              dataSource.getAssetSeries(slide.assetSeriesId).assetIds[slide.coverIndex],
            )}
      />
    {/if}
  </div>
</div>

<style>
  .slide-holder {
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;

    display: block;
    z-index: 1;
    overflow: hidden;
    box-sizing: border-box;
  }
</style>
