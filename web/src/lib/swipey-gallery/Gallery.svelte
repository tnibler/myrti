<script lang="ts">
  import { onMount } from 'svelte';
  import Pager, { type PagerProps } from './Pager.svelte';

  type GalleryProps = PagerProps & {
    scrollWrapper: HTMLElement;
    restoreScrollOnClose: boolean;
  };

  let {
    numSlides,
    getSlide,
    getThumbnailBounds,
    scrollWrapper = $bindable(),
    restoreScrollOnClose,
  }: GalleryProps = $props();
  let isOpen: boolean = $state(false);
  let slideIndex = $state(0);
  let pager: Pager | null = $state(null);
  let pagerY = 0;
  let topOffset = $state(0);

  function onKeyDown(e: KeyboardEvent) {
    console.assert(isOpen);
    if (e.key === 'ArrowLeft') {
      pager?.moveSlide('left');
    } else if (e.key === 'ArrowRight') {
      pager?.moveSlide('right');
    } else if (e.key === 'Escape') {
      close();
    }
  }

  onMount(() => shakaInit());

  function onOpenTransitionFinished() {}

  export function setIndex(index: number) {
    slideIndex = index;
  }

  export function open(index: number) {
    requestAnimationFrame(() => {
      pagerY = scrollWrapper.scrollTop;
      scrollWrapper.classList.add('modalOpen');
      topOffset = 0;
      scrollWrapper.scrollTo(0, pagerY);
    });
    slideIndex = index;
    topOffset = scrollWrapper.scrollTop;
    isOpen = true;
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

{#if isOpen}
  <Pager
    {numSlides}
    {getSlide}
    {getThumbnailBounds}
    {onOpenTransitionFinished}
    closeGallery={close}
    bind:slideIndex
    bind:this={pager}
    {topOffset}
  />
{/if}

<style>
  :global(.modalOpen) {
    overflow: hidden;
  }
</style>
