<script lang="ts">
	import Slide, {
		type OpenTransitionParams,
		type SlideControls,
	} from "./Slide.svelte";
	import type { SlideData } from "./slide-data";

	type SlideHolderProps = {
		isActive: boolean;
		xTransform: number;
		id: number;
		slide: Promise<SlideData> | null;
		onContentReady: (() => void) | undefined;
		showContent: boolean;
		slideControls: SlideControls;
		openTransition: OpenTransitionParams | null;
	};
	let {
		isActive,
		xTransform,
		id,
		slide,
		onContentReady,
		showContent,
		openTransition,
	}: SlideHolderProps = $props();
	let slideComponent: Slide | null = $state(null);
	export const controls: SlideControls | null = $derived(
		slideComponent?.controls,
	);
	const transformStr: string = $derived(
		`translate3d(${Math.round(xTransform)}px, 0px, 0px)`,
	);
</script>

<div id="id-{id}" class="item" style="transform: {transformStr};">
	{#await slide then awaitedSlide}
		{#if awaitedSlide !== null}
			<Slide
				data={awaitedSlide}
				{isActive}
				{openTransition}
				{onContentReady}
				{showContent}
				bind:this={slideComponent}
			/>
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
