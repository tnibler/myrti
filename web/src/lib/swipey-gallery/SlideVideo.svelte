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
  };

  const { size, slideData, isVisible, isActive, onContentReady }: SlideVideoProps = $props();

  let isCloseTransitionRunning = $state(false);
  let videoEl: HTMLVideoElement | undefined = $state();
  let enableVideoSrcOrig: { src: string; mimeType: string } | null = $state(null);

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
    if (slideData.videoSource === 'dash') {
      shakaInitPlayer(slideData.mpdManifestUrl);
    } else {
      enableVideoSrcOrig = slideData;
    }
    setTimeout(() => {
      if (videoEl) {
        videoEl.controls = true;
        if (isActive) {
          videoEl.play();
        }
      }
    }, 400);
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

  async function shakaInitPlayer(mpdManifestUrl: string) {
    const player = new shaka.Player();
    await player.attach(videoEl);
    await player.load(mpdManifestUrl);
  }
</script>

<video
  autoplay={isActive}
  muted={true}
  class="slide-video max-w-none"
  bind:this={videoEl}
  onloadeddata={onContentReady}
  width={size.width}
  style:width="{size.width}px"
  style:height="{size.height}px"
  style:user-select="none"
  class:slide-transition-transform={isCloseTransitionRunning}
  class:slide-transition-opacity={!isCloseTransitionRunning}
  class:hidden={!isVisible}
>
  {#if enableVideoSrcOrig !== null}
    <source
      src={enableVideoSrcOrig.src}
      type={enableVideoSrcOrig.mimeType}
      onerror={(e) => {
        console.log('TODO handle video codec errors', e);
        onContentReady();
      }}
    />
  {/if}
</video>

<style>
  .slide-video {
    position: absolute;
  }

  .hidden {
    display: none;
  }
</style>
