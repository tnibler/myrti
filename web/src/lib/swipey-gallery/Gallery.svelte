<script lang="ts">
	import { onMount } from 'svelte';
	import Pager, { type PagerProps } from './Pager.svelte';

	type GalleryProps = PagerProps & { scrollWrapper: HTMLElement };

	let { numSlides, getSlide, getThumbnailBounds, scrollWrapper } = $props<GalleryProps>();
	let isOpen: boolean = $state(false);
	let slideIndex = $state(0);
	let pager: Pager;
	let pagerY = 0;
	let topOffset = $state(0);

	onMount(() => shakaInit());

	function onOpenTransitionFinished() {}

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
	}

	export function close() {
		pager.close().then(() => {
			isOpen = false;
			scrollWrapper.classList.remove('modalOpen');
			scrollWrapper.style.height = '100%';
			requestAnimationFrame(() => {
				scrollWrapper.scrollTo(0, pagerY);
			});
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
