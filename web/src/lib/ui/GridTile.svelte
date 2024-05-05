<script context="module">
	export type TileBox = {
		width: number;
		height: number;
		top: number;
		left: number;
	};
</script>

<script lang="ts">
	import type { Asset } from '$lib/apitypes';
	import {
		mdiProgressWrench,
		mdiPlayCircleOutline,
		mdiCheckCircle,
		mdiCheckCircleOutline,
		mdiCheckboxMarkedCircle,
		mdiCircleOutline
	} from '@mdi/js';
	import { fade } from 'svelte/transition';

	type GridTileProps = {
		asset: Asset;
		box: TileBox;
		selectState: { inSelectMode: false } | { inSelectMode: true; isSelected: boolean };
		onSelectToggled: () => void;
		onAssetClick: () => void;
		imgEl: HTMLImageElement;
	};
	let {
		asset,
		box,
		selectState,
		onSelectToggled,
		onAssetClick,
		imgEl = $bindable()
	}: GridTileProps = $props();
	let isMouseOver = $state(false);
	const isSelected = $derived(selectState.inSelectMode && selectState.isSelected);

	function onSelectButtonClick() {
		onSelectToggled();
	}

	function onTileClick() {
		if (selectState.inSelectMode) {
			onSelectToggled();
		} else {
			onAssetClick();
		}
	}
</script>

<a
	href="#"
	class="absolute group select-none"
	style="width: {box.width}px; height: {box.height}px; top: {box.top}px; left: {box.left}px; "
	onclick={(e) => {
		e.preventDefault();
		onTileClick();
	}}
	onmouseenter={() => {
		isMouseOver = true;
	}}
	onmouseleave={() => {
		isMouseOver = false;
	}}
>
	<div class="h-full w-full bg-blue-100">
		<!-- svelte-ignore a11y-missing-attribute -->
		<img
			bind:this={imgEl}
			src="/api/asset/thumbnail/{asset.id}/large/avif"
			height={box.width}
			width={box.height}
			class="absolute bg-black transition-transform"
			class:rounded-xl={isSelected}
			class:scale-[0.85]={isSelected}
			style:transform-origin="top left"
			style:left={box.width + 'px'}
			style:rotate="90deg"
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
			{#if selectState.inSelectMode || isMouseOver}
				{@const icon = isSelected
					? mdiCheckboxMarkedCircle
					: selectState.inSelectMode
						? mdiCircleOutline
						: mdiCheckCircleOutline}
				<button
					class="absolute left-0 p-1 md:p-2 focus:outline-none"
					role="checkbox"
					aria-checked={isSelected}
					onclick={(e) => {
						e.stopPropagation();
						e.preventDefault();
						onSelectButtonClick();
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

<style>
	img {
		max-width: none;
	}
</style>
