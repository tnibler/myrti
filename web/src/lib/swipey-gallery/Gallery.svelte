<script lang="ts">
  import { onMount } from 'svelte';
  import Pager, { type PagerProps } from './Pager.svelte';

  type GalleryProps = Omit<
    PagerProps,
    'topOffset' | 'closeGallery' | 'onOpenTransitionFinished'
  > & {
    scrollWrapper: HTMLElement;
    restoreScrollOnClose: boolean;
    isOpen: boolean;
    closeGallery: () => void;
  };

  let {
    scrollWrapper = $bindable(),
    isOpen,
    restoreScrollOnClose,
    closeGallery,
    ...pagerProps
  }: GalleryProps = $props();
  let actuallyOpen: 'open' | 'closed' | 'closing' = $state('closed');
  let pager: Pager | null = $state(null);
  let pagerY = 0;
  let topOffset = $state(0);

  function onKeyDown(e: KeyboardEvent) {
    if (isOpen !== false) {
      if (e.key === 'ArrowLeft') {
        pager?.moveSlide('left');
      } else if (e.key === 'ArrowRight') {
        pager?.moveSlide('right');
      } else if (e.key === 'Escape') {
        closeGallery();
        // isOpen = false;
      }
    }
  }

  onMount(() => shakaInit());

  function onOpenTransitionFinished() {}

  $effect(() => {
    if (actuallyOpen === 'open' && !isOpen) {
      actuallyOpen = 'closing';
      closeAnim();
    } else if (actuallyOpen === 'closed' && isOpen) {
      actuallyOpen = 'open';
      open();
    } else if (actuallyOpen === 'open' && isOpen) {
      return;
    } else if (actuallyOpen === 'closed' && !isOpen) {
      return;
    }
  });

  function open() {
    requestAnimationFrame(() => {
      pagerY = scrollWrapper.scrollTop;
      scrollWrapper.classList.add('modalOpen');
      topOffset = 0;
      scrollWrapper.scrollTo(0, pagerY);
    });
    actuallyOpen = 'open';
    topOffset = scrollWrapper.scrollTop;
    document.addEventListener('keydown', onKeyDown);
  }

  function closeAnim() {
    document.removeEventListener('keydown', onKeyDown);
    pager?.close().then(() => {
      actuallyOpen = 'closed';
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

{#if actuallyOpen !== 'closed'}
  <Pager bind:this={pager} {...pagerProps} {topOffset} {onOpenTransitionFinished} {closeGallery} />
{/if}

<style>
  :global(.modalOpen) {
    overflow: hidden;
  }
</style>
