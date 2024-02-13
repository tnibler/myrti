<script lang="ts">
	import Pager, { type PagerProps } from './Pager.svelte';

	type GalleryProps = PagerProps & { bodyWrapper: HTMLElement };

	let { numSlides, getSlide, getThumbnailBounds, bodyWrapper } = $props<GalleryProps>();
	let isOpen: boolean = $state(false);
	let slideIndex = $state(0);
	let pager: Pager;
	let pagerY = 0;
	let topOffset = $state(0);

	function onOpenTransitionFinished() {}

	export function open(index: number) {
		requestAnimationFrame(() => {
			pagerY = window.scrollY;
			bodyWrapper.classList.add('modalOpen');
			bodyWrapper.style.height = '100vh';
			topOffset = 0;
			bodyWrapper.scrollTo(0, pagerY);
		});
		slideIndex = index;
		topOffset = window.scrollY;
		isOpen = true;
	}

	export function close() {
		pager.close().then(() => {
			isOpen = false;
			bodyWrapper.classList.remove('modalOpen');
			bodyWrapper.style.height = '100%';
			requestAnimationFrame(() => {
				window.scrollTo(0, pagerY);
			});
		});
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
