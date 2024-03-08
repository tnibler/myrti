<script lang="ts">
	import type { Size } from './util_types';
	import type { ImageSlideData } from './slide-data';
	import './slide.css';

	type SlideImageProps = {
		/** size of the DOM element */
		size: Size;
		/** Callback when image/video is loaded and the placeholder should disappear */
		slideData: ImageSlideData;
		isVisible: boolean;
		onContentReady: () => void;
	};

	const { size, slideData, isVisible, onContentReady } = $props<SlideImageProps>();

	let isCloseTransitionRunning = $state(false);
	let imgEl: HTMLImageElement | undefined = $state();

	export function closeTransition(transform: string, onTransitionEnd: () => void) {
		if (!imgEl) {
			console.error('SlideImage.closeTransition called, but <img> element is not bound');
			return;
		}
		const listener = (e: TransitionEvent) => {
			if (e.target === imgEl) {
				imgEl.removeEventListener('transitionend', listener, false);
				imgEl.removeEventListener('transitioncancel', listener, false);
				isCloseTransitionRunning = false;
				onTransitionEnd();
			}
		};
		imgEl.addEventListener('transitionend', listener, false);
		imgEl.addEventListener('transitioncancel', listener, false);

		isCloseTransitionRunning = true;
		requestAnimationFrame(() => {
			if (!imgEl) {
				return;
			}
			imgEl.style.transform = transform;
		});
	}
</script>

<!-- svelte-ignore a11y-missing-attribute -->
<img
	class="slide-image"
	bind:this={imgEl}
	src={slideData.src}
	onload={onContentReady}
	onerror={(e) => {
		console.log('error loading image', e);
		onContentReady();
	}}
	decoding="async"
	style:width="{size.width}px"
	style:height="{size.height}px"
	style:user-select="none"
	class:slide-transition-transform={isCloseTransitionRunning}
	class:slide-transition-opacity={!isCloseTransitionRunning}
	class:hidden={!isVisible}
/>

<style>
	.slide-image {
		position: absolute;
		max-width: none; /* override tailwind */
	}

	.slide-image.hidden {
		display: none;
	}
</style>
