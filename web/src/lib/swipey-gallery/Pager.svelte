<script lang="ts" context="module">
  import type { GallerySlide } from './gallery-types';
  export type PagerProps<TPos> = {
    topOffset: number;
    getSlide: (pos: TPos) => Promise<GallerySlide<TPos>>;
    getNextSlidePosition: (pos: TPos, dir: 'left' | 'right') => TPos | null;
    currentPosition: TPos;
    getThumbnailBounds: (pos: TPos) => ThumbnailBounds;
    closeGallery: () => void;
    onOpenTransitionFinished: () => void;
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

<script lang="ts" generics="TPos">
  import Slide from './Slide.svelte';
  import SlideHolder from './SlideHolder.svelte';
  import { onMount, setContext } from 'svelte';
  import { newGestureController } from './gestures';
  import { newAnimationControls, type AnimationControls } from './animations';
  import type { ThumbnailBounds } from './thumbnail-bounds';
  import type { OpenTransitionParams, SlideControls } from './Slide.svelte';
  import { fade } from 'svelte/transition';
  import {
    EyeOffIcon,
    InfoIcon,
    RotateCwIcon,
    XIcon,
    ZoomInIcon,
    ZoomOutIcon,
  } from 'lucide-svelte';
  import InfoPanel from './InfoPanel.svelte';

  let {
    getSlide,
    getNextSlidePosition,
    getThumbnailBounds,
    closeGallery,
    onOpenTransitionFinished,
    topOffset,
    currentPosition = $bindable(),
  }: PagerProps<TPos> = $props();

  let viewport = $state({ width: 0, height: 0 });
  const slideSpacing = 0.1;
  const slideWidth = $derived(viewport.width + viewport.width * slideSpacing);

  /** used for transform/offset calculations */
  let currentShift = $state(0);
  let isSidePanelOpen: boolean = $state(false);

  type SlideHolderState = {
    id: number;
    slidePosition: TPos | null;
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
      const positions = [
        getNextSlidePosition(currentPosition, 'left'),
        currentPosition,
        getNextSlidePosition(currentPosition, 'right'),
      ];
      const openTransition = {
        onTransitionEnd: afterOpenTransition,
        fromBounds: getThumbnailBounds(currentPosition),
      };
      // holderOrder is the identity mapping at the beginning, so id == index initially for the SlideHolders
      return [0, 1, 2].map((id) => {
        return {
          // maybe hide left and right holders until open anim finished? see main-scroll.js:111
          id: id,
          slidePosition: positions[id],
          openTransition: id === 1 ? openTransition : null,
          isActive: id === 1,
          showContent: id === 1,
          isContentReady: false,
        };
      });
    })(),
  );
  const currentSlide: Promise<GallerySlide<TPos>> | null = $derived.by(() => {
    if (holderOrder[1] === undefined || holderStates[holderOrder[1]] === undefined) {
      return null;
    }
    const slidePos = holderStates[holderOrder[1]].slidePosition;
    if (slidePos === null) {
      return null;
    }
    return getSlide(slidePos);
  });
  const canMoveLeft = $derived(holderStates[holderOrder[0]].slidePosition !== null);
  const canMoveRight = $derived(holderStates[holderOrder[2]].slidePosition !== null);
  let slideHolders: SlideHolder[] = $state([]);
  const xTransformSlideCenter = $derived(-currentShift * slideWidth * (1 + slideSpacing));
  let xTransformOffset = $state(0);
  let xTransform = $derived(xTransformSlideCenter + xTransformOffset);
  const transformString = $derived(`translate3d(${Math.round(xTransform)}px, 0px, 0px)`);
  let backgroundOpacity = $state(0);
  /** enable CSS transition when assigning backgroundOpacity. Only set on open and close. */
  let backgroundOpacityTransition = $state(true);

  let hasMouse = $state(false);

  const animations: AnimationControls = newAnimationControls();
  const slide: SlideControls | null = $derived(
    holderOrder[1] < slideHolders.length ? slideHolders[holderOrder[1]]?.slideControls : null,
  );
  const hideUiTimeoutDuration = 3000;
  let hideUiTimeout: ReturnType<typeof setTimeout> | null = setTimeout(
    onHideUiTimeout,
    hideUiTimeoutDuration,
  );
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
      const hittingLeftWall = 0 < delta && holderStates[holderOrder[0]].slidePosition === null;
      const hittingRightWall = delta < 0 && holderStates[holderOrder[2]].slidePosition === null;
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
      backgroundOpacity = 1 - ratio;
    },
  };
  setContext('gallery', gallery);

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
    const onMouseDetected = () => {
      hasMouse = true;
    };
    let gestureController = newGestureController(gallery, onMouseDetected);
    pagerWrapper.onpointerdown = (e) => {
      if (!uiVisible) {
        showUi();
        // don't initiate drag or anything if ui was hidden
        return;
      }
      gestureController.onPointerDown(e);
    };
    window.onpointerup = gestureController.onPointerUp;
    window.onpointermove = gestureController.onPointerMove;
    pagerWrapper.onpointercancel = gestureController.onPointerUp;
    pagerWrapper.onclick = gestureController.onClick;
    document.documentElement.onpointerleave = gestureController.onPointerUp;
    window.onmousemove = () => {
      showUi();
    };
  }

  function unbindEvents() {
    pagerWrapper.onpointerdown = null;
    window.onpointerup = null;
    window.onpointermove = null;
    pagerWrapper.onpointercancel = null;
    pagerWrapper.onclick = null;
    window.onmousemove = null;
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
        start: 0,
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
      newActiveHolder.slidePosition !== null,
      'newActiveHolder.slidePosition is null after shuffling SlideHolders',
    );
    if (newActiveHolder.slidePosition !== null) {
      currentPosition = newActiveHolder.slidePosition;
      movedHolder.slidePosition = getNextSlidePosition(currentPosition, didShift);
      movedHolder.openTransition = null;
      movedHolder.isContentReady = false;
    }
  }

  export async function close() {
    const thumbnailBounds = getThumbnailBounds(currentPosition);
    backgroundOpacityTransition = true;
    // requestAnimationFrame(() => {
    backgroundOpacity = 0;
    // });
    const p = new Promise<void>((resolve) => {
      if (slide) {
        slide.closeTransition(thumbnailBounds, () => {
          resolve();
        });
      } else {
        resolve();
      }
    });
    return p;
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
  absolute top-0 left-0 w-full h-dvh
  flex flex-row
  touch-none overflow-hidden z-5"
  style:cursor={uiVisible ? 'default' : 'none'}
  style:top={`${topOffset}px`}
>
  <div
    class="grow relative z-[1000]"
    bind:this={pagerWrapper}
    bind:clientHeight={viewport.height}
    bind:clientWidth={viewport.width}
  >
    <div
      class="w-full h-full top-0 left-0 bg-black z-0 transition-opacity duration-200 ease-in-out"
      style:opacity={backgroundOpacity}
      class:transition-opacity={backgroundOpacityTransition}
    ></div>
    <div class="absolute top-0 left-0 w-full h-full" style="transform: {transformString};">
      {#each holderStates as slideHolder (slideHolder.id)}
        <!-- currentShift - 1 because there is still one slideHolder to the left of the viewport when currentShift is 0 -->
        {@const x =
          (currentShift - 1 + holderOrder.indexOf(slideHolder.id)) *
          (1 + slideSpacing) *
          slideWidth}
        <SlideHolder
          id={slideHolder.id}
          isActive={slideHolder.isActive}
          xTransform={x}
          openTransition={slideHolder.openTransition}
          showContent={slideHolder.showContent}
          onContentReady={() => onSlideContentReady(slideHolder.id)}
          slide={slideHolder.slidePosition !== null ? getSlide(slideHolder.slidePosition) : null}
          bind:this={slideHolders[slideHolder.id]}
        />
      {/each}
    </div>
    {#if uiVisible}
      <!-- Note: idk what capture really means at time of writing. The intent is for the pointerdown/up/.. listeners in bindEvent()
        to not be triggered when ui elements in this div are clicked. -->
      <div
        class="absolute top-0 left-0 w-full h-full flex flex-col z-10 pointer-events-none"
        out:fade
        onpointerdowncapture={(e) => {
          e.stopPropagation();
        }}
        onpointerupcapture={(e) => {
          e.stopPropagation();
        }}
      >
        <div
          class="flex flex-row flex-shrink justify-end items-center
    h-16 px-2 gap-4 bg-gradient-to-b from-black/50 pointer-events-auto"
        >
          <button class="p-2" class:button-visible={hasMouse} onclick={() => {}}>
            <RotateCwIcon color="white" />
          </button>
          <button
            class="p-2"
            class:button-visible={hasMouse}
            onclick={() => onZoomOutClicked()}
            disabled={isZoomOutDisabled}
          >
            <ZoomOutIcon color={isZoomOutDisabled ? '#aaa' : 'white'} />
          </button>
          <button
            class="p-2"
            class:button-visible={hasMouse}
            onclick={() => onZoomInClicked()}
            disabled={isZoomInDisabled}
          >
            <ZoomInIcon color={isZoomInDisabled ? '#aaa' : 'white'} />
          </button>
          <button class="p-2" class:button-visible={hasMouse} onclick={() => {}}>
            <EyeOffIcon color="white" />
          </button>
          <button
            class="p-2"
            class:button-visible={hasMouse}
            onclick={() => {
              isSidePanelOpen = !isSidePanelOpen;
              moveSlideAnimate('backToCenter');
            }}
          >
            <InfoIcon color="white" />
          </button>
          <button class="p-4" class:button-visible={hasMouse} onclick={() => closeGallery()}>
            <XIcon color="white" />
          </button>
        </div>
        <div class="flex flex-row flex-grow justify-between {hasMouse ? '' : 'hidden'} ">
          <button
            class="pl-5 pointer-events-auto"
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
            class="pr-5 pointer-events-auto"
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

  <div class={'bg-white z-50 transition-all w-96 ' + (isSidePanelOpen ? 'mr-0' : 'mr-[-24rem]')}>
    {#await currentSlide then slide}
      {#if slide !== null}
        <InfoPanel
          asset={slide.slideType === 'singleAsset' ? slide.asset : slide.coverSlide.asset}
        />
      {/if}
    {/await}
  </div>
</div>

<style>
  .slide-container {
    user-select: none;
  }
</style>
