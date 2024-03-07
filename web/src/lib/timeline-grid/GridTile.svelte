<script lang="ts">
	import type { Asset } from '$lib/apitypes';
	import type { TimelineGridStore } from '$lib/store/timeline.svelte';
	import { mdiCheckCircle, mdiCheckCircleOutline } from '@mdi/js';
	import type { TileBox } from './GridSegment.svelte';

	type GridTileProps = {
		assetIndex: number;
		asset: Asset;
		box: TileBox;
		timeline: TimelineGridStore;
		inSelectionMode: boolean;
		onAssetClick: () => void;
		imgEl: HTMLImageElement;
	};
	let { assetIndex, asset, box, timeline, inSelectionMode, onAssetClick, imgEl } =
		$props<GridTileProps>();
	const isSelected = $derived(assetIndex in timeline.selectedAssetIndices);
	let isMouseOver = $state(false);
</script>

<a
	href="#"
	onclick={(e) => {
		e.preventDefault();
		if (inSelectionMode) {
			timeline.setAssetSelected(assetIndex, !isSelected);
		} else {
			onAssetClick();
		}
	}}
	onmouseenter={() => {
		isMouseOver = true;
	}}
	onmouseleave={() => {
		isMouseOver = false;
	}}
>
	<div>
		<!-- svelte-ignore a11y-missing-attribute -->
		<img
			bind:this={imgEl}
			src="/api/asset/thumbnail/{asset.id}/large/avif"
			class="absolute bg-black"
			style="width: {box.width}px; height: {box.height}px; top: {box.top}px; left: {box.left}px; z-index: 10;"
		/>
		<div
			style="position: absolute; width: {box.width}px; height: {box.height}px; top: {box.top}px; left: {box.left}px; z-index: 20;"
		>
			{#if inSelectionMode || isMouseOver}
				<button
					class="absolute p-2 focus:outline-none"
					role="checkbox"
					aria-checked="false"
					onclick={(e) => {
						e.stopPropagation();
						timeline.setAssetSelected(assetIndex, !isSelected);
					}}
				>
					<svg style="opacity: 0.6;" width="24" height="24" viewBox="0 0 24 24"
						><path d={isSelected ? mdiCheckCircle : mdiCheckCircleOutline} fill="#fff" /></svg
					>
				</button>
			{/if}
		</div>
	</div>
	<div
		class="absolute z-10 h-full w-full bg-gradient-to-b from-black/25 via-[transparent_25%] opacity-0 transition-opacity group-hover:opacity-100"
	></div>
</a>
