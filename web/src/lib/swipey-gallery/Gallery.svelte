<script lang="ts">
	import { onMount } from 'svelte';
	import Pager, { type PagerProps } from './Pager.svelte';

	type GalleryProps = PagerProps & { bodyWrapper: HTMLElement };

	let { numSlides, getSlide, getThumbnailBounds, bodyWrapper } = $props<GalleryProps>();
	let isOpen: boolean = $state(false);
	let slideIndex = $state(0);
	let pager: Pager;
	let pagerY = 0;
	let topOffset = $state(0);

	onMount(() => shakaInit());

	function onOpenTransitionFinished() {}

	export function open(index: number) {
		requestAnimationFrame(() => {
			pagerY = bodyWrapper.scrollTop;
			bodyWrapper.classList.add('modalOpen');
			topOffset = 0;
			bodyWrapper.scrollTo(0, pagerY);
		});
		slideIndex = index;
		topOffset = bodyWrapper.scrollTop;
		isOpen = true;
	}

	export function close() {
		pager.close().then(() => {
			isOpen = false;
			bodyWrapper.classList.remove('modalOpen');
			bodyWrapper.style.height = '100%';
			requestAnimationFrame(() => {
				bodyWrapper.scrollTo(0, pagerY);
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
