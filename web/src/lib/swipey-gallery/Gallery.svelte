<script lang="ts" generics="TPos">
  import { onMount } from 'svelte';
  import Pager, { type PagerProps } from './Pager.svelte';

  type GalleryProps<TPos> = PagerProps<TPos> & {
    scrollWrapper: HTMLElement;
    restoreScrollOnClose: boolean;
  };

  let {
    numSlides,
    getSlide,
    getNextSlidePosition,
    getThumbnailBounds,
    scrollWrapper = $bindable(),
    restoreScrollOnClose,
  }: GalleryProps<TPos> = $props();
  let isOpen: false | { currentPosition: TPos } = $state(false);
  let pager: Pager<TPos> | null = $state(null);
  let pagerY = 0;
  let topOffset = $state(0);

  function onKeyDown(e: KeyboardEvent) {
    if (isOpen !== false) {
      if (e.key === 'ArrowLeft') {
        const newPos = getNextSlidePosition(isOpen.currentPosition, 'left');
        if (newPos !== null) {
          // isOpen.currentPosition = newPos;
          pager?.moveSlide('left');
        }
      } else if (e.key === 'ArrowRight') {
        const newPos = getNextSlidePosition(isOpen.currentPosition, 'right');
        if (newPos !== null) {
          // isOpen.currentPosition = newPos;
          pager?.moveSlide('right');
        }
      } else if (e.key === 'Escape') {
        close();
      }
    }
  }

  onMount(() => shakaInit());

  function onOpenTransitionFinished() {}

  export function setPosition(pos: TPos) {
    if (isOpen !== false) {
      isOpen.currentPosition = pos;
    }
  }

  export function open(pos: TPos) {
    requestAnimationFrame(() => {
      pagerY = scrollWrapper.scrollTop;
      scrollWrapper.classList.add('modalOpen');
      topOffset = 0;
      scrollWrapper.scrollTo(0, pagerY);
    });
    topOffset = scrollWrapper.scrollTop;
    isOpen = { currentPosition: pos };
    document.addEventListener('keydown', onKeyDown);
  }

  export function close() {
    document.removeEventListener('keydown', onKeyDown);
    pager?.close().then(() => {
      isOpen = false;
      scrollWrapper.classList.remove('modalOpen');
      scrollWrapper.style.height = '100%';
      if (restoreScrollOnClose) {
        requestAnimationFrame(() => {
          scrollWrapper.scrollTo(0, pagerY);
        });
      }
    });
  }

  function shakaInit() {
    if (window.shaka) {
      return;
    }
    shaka.polyfill.installAll();
    if (!shaka.Player.isBrowserSupported()) {
      console.error('shaka player not supported in this browser');
      return;
    }
    window.shaka = shaka;
  }
</script>

{#if isOpen !== false}
  <Pager
    {numSlides}
    {getSlide}
    {getThumbnailBounds}
    {onOpenTransitionFinished}
    {getNextSlidePosition}
    bind:currentPosition={isOpen.currentPosition}
    closeGallery={close}
    bind:this={pager}
    {topOffset}
  />
{/if}

<style>
  :global(.modalOpen) {
    overflow: hidden;
  }
</style>
