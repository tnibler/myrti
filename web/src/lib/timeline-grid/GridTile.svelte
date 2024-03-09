<script lang="ts">
	import type { Asset } from '$lib/apitypes';
	import type { TimelineGridStore } from '$lib/store/timeline.svelte';
	import {
		mdiProgressWrench,
		mdiPlayCircleOutline,
		mdiCheckCircle,
		mdiCheckCircleOutline,
		mdiCheckboxMarkedCircle,
		mdiCircleOutline
	} from '@mdi/js';
	import type { TileBox } from './GridSegment.svelte';
	import { fade } from 'svelte/transition';

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
	class="absolute group select-none"
	style="width: {box.width}px; height: {box.height}px; top: {box.top}px; left: {box.left}px;"
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
	<div class="h-full h-full bg-blue-100">
		<!-- svelte-ignore a11y-missing-attribute -->
		<img
			bind:this={imgEl}
			src="/api/asset/thumbnail/{asset.id}/large/avif"
			class="absolute bg-black h-full w-full transition-transform"
			class:rounded-xl={isSelected}
			class:scale-[0.85]={isSelected}
		/>
		<div
			class="absolute z-10 h-full w-full bg-gradient-to-b from-black/25 via-[transparent_25%] opacity-0 transition-opacity group-hover:opacity-100"
			class:rounded-xl={isSelected}
			class:scale-[0.85]={isSelected}
		/>
		{#if asset.type === 'video'}
			{@const icon = asset.hasDash ? mdiPlayCircleOutline : mdiProgressWrench}
			<svg
				class="absolute right-0 mr-1 mt-1 md:mr-2 md:mt-2"
				style="opacity: 0.75;"
				width="24"
				height="24"
				viewBox="0 0 24 24"
			>
				<path d={icon} fill="#fff" />
			</svg>
		{/if}
		<div class="absolute z-20 h-full w-full">
			{#if inSelectionMode || isMouseOver}
				{@const icon = isSelected
					? mdiCheckboxMarkedCircle
					: inSelectionMode
						? mdiCircleOutline
						: mdiCheckCircleOutline}
				<button
					class="absolute left-0 p-1 md:p-2 focus:outline-none"
					role="checkbox"
					aria-checked={isSelected}
					onclick={(e) => {
						e.stopPropagation();
						e.preventDefault();
						timeline.setAssetSelected(assetIndex, !isSelected);
					}}
					transition:fade={{ duration: 80 }}
				>
					<svg style:opacity={isSelected ? 1 : 0.75} width="24" height="24" viewBox="0 0 24 24"
						><path d={icon} fill="#fff" />
					</svg>
				</button>
			{/if}
		</div>
	</div>
</a>
