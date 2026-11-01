<script lang="ts">
  import type { Size } from './util_types';
  import './slide.css';
  import type { VideoSlideData } from './gallery-types';

  type SlideVideoProps = {
    /** size of the DOM element */
    size: Size;
    /** Callback when image/video is loaded and the placeholder should disappear */
    slideData: VideoSlideData;
    isVisible: boolean;
    isActive: boolean;
    onContentReady: () => void;
    placeholderTransitionState: 'No' | 'Running' | 'Finished';
  };

  const {
    size,
    slideData,
    isVisible,
    isActive,
    onContentReady,
    placeholderTransitionState,
  }: SlideVideoProps = $props();

  let isCloseTransitionRunning = $state(false);
  let videoEl: HTMLVideoElement | undefined = $state();
  let videoContainerEl: HTMLElement | undefined = $state();

  $effect(() => {
    if (!videoEl) {
      return;
    }
    if (isActive) {
      videoEl.play();
    } else {
      videoEl.pause();
    }
  });
  $effect(() => {
    if (!isActive) {
      return;
    }
    shakaInitPlayer();
  });

  export function closeTransition(transform: string, onTransitionEnd: () => void) {
    if (!videoEl) {
      console.error('SlideVideo.closeTransition called, but <video> element is not bound');
      return;
    }
    const listener = (e: TransitionEvent) => {
      if (e.target === videoEl) {
        videoEl.removeEventListener('transitionend', listener, false);
        videoEl.removeEventListener('transitioncancel', listener, false);
        isCloseTransitionRunning = false;
        onTransitionEnd();
      }
    };
    videoEl.addEventListener('transitionend', listener, false);
    videoEl.addEventListener('transitioncancel', listener, false);

    isCloseTransitionRunning = true;
    requestAnimationFrame(() => {
      if (!videoEl) {
        return;
      }
      videoEl.style.transform = transform;
    });
  }

  let uiControls = null;
  let player = null;
  $effect(() => {
    if (!isActive || placeholderTransitionState !== 'Finished') {
      return;
    }
    if (!videoEl || !player || uiControls) {
      return;
    }
    const ui = new shaka.ui.Overlay(player, videoContainerEl, videoEl);
    uiControls = ui.getControls();
    ui.configure({
      topControlPanelElements: [],
      controlPanelElements: [
        'play_pause',
        'mute_volume',
        'time_and_duration',
        'spacer',
        'quality',
        'fullscreen',
      ],
      bigButtons: ['play_pause'],
      enableTooltips: true,
      // doubleClickForFullscreen: false,
      // seekOnTaps: true,
    });
    uiControls.addEventListener('error', console.log);
  });

  async function shakaInitPlayer() {
    if (!videoEl || !videoContainerEl) {
      return;
    }
    player = new shaka.Player();
    await player.attach(videoEl);
    if (slideData.videoSource === 'original') {
      await player.load(slideData.src);
    } else {
      await player.load(slideData.mpdManifestUrl);
    }
  }
</script>

{#if isActive}
  <!-- this wrapper is bc shaka will set position: relative on the container div -->
  <div class="absolute" style:width="{size.width}px" style:height="{size.height}px">
    <div
      class="flex flex-row items-center"
      bind:this={videoContainerEl}
      data-gesture-noclick-recursive
    >
      <video
        autoplay={isActive}
        class="flex-1"
        bind:this={videoEl}
        onloadeddata={onContentReady}
        width={size.width}
        style:width="{size.width}px"
        style:height="{size.height}px"
        style:user-select="none"
        class:slide-transition-transform={isCloseTransitionRunning}
        class:slide-transition-opacity={!isCloseTransitionRunning}
        class:invisible={!isVisible}
      >
      </video>
    </div>
  </div>
{/if}

<style>
</style>
