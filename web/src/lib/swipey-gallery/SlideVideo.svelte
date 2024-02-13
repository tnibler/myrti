<script lang="ts">
	import type { Size } from './util_types';
	import type { VideoSlideData } from './slide-data';
	import './slide.css';

	type SlideVideoProps = {
		/** size of the DOM element */
		size: Size;
		/** Callback when image/video is loaded and the placeholder should disappear */
		slideData: VideoSlideData;
		isVisible: boolean;
		isActive: boolean;
		onContentReady: () => void;
	};

	const { size, slideData, isVisible, isActive, onContentReady } = $props<SlideVideoProps>();

	let isCloseTransitionRunning = $state(false);
	let videoEl: HTMLVideoElement | undefined = $state();

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
		slideData.src;
		setTimeout(() => {
			if (videoEl) {
				videoEl.controls = true;
				videoEl.play();
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
</script>

<video
	autoplay={isActive}
	muted={true}
	class="slide-video"
	bind:this={videoEl}
	on:loadeddata={onContentReady}
	width={size.width}
	style:width="{size.width}px"
	style:height="{size.height}px"
	style:user-select="none"
	class:slide-transition-transform={isCloseTransitionRunning}
	class:slide-transition-opacity={!isCloseTransitionRunning}
	class:hidden={!isVisible}
>
	<source src={slideData.src} type={slideData.mimeType} />
</video>

<style>
	.slide-video {
		position: absolute;
	}

	.hidden {
		display: none;
	}
</style>
