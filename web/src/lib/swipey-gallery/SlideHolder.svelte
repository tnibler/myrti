<script lang="ts">
	import Slide, { type OpenTransitionParams, type SlideControls } from './Slide.svelte';
	import type { SlideData } from './slide-data';

	type SlideHolderProps = {
		isActive: boolean;
		xTransform: number;
		id: number;
		slide: Promise<SlideData> | null;
		slideControls: SlideControls;
		openTransition: OpenTransitionParams | null;
	};
	let { isActive, xTransform, id, slide, slideControls, openTransition } =
		$props<SlideHolderProps>();
	const transformStr: string = $derived(`translate3d(${Math.round(xTransform)}px, 0px, 0px)`);
</script>

<div id="id-{id}" class="item" style="transform: {transformStr};">
	{#await slide then awaitedSlide}
		{#if awaitedSlide !== null}
			<Slide data={awaitedSlide} {openTransition} bind:controls={slideControls} />
		{/if}
	{/await}
</div>

<style>
	.item {
		position: absolute;
		top: 0;
		left: 0;
		width: 100%;
		height: 100%;

		display: block;
		z-index: 1;
		overflow: hidden;
		box-sizing: border-box;
	}
</style>
