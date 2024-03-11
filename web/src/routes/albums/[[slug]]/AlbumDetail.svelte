<script lang="ts">
	import { api } from '$lib/apiclient';
	import type { Asset } from '$lib/apitypes';
	import type { TileBox } from '$lib/timeline-grid/GridSegment.svelte';
	import createJustifiedLayout from 'justified-layout';
	import { onMount } from 'svelte';

	type Props = {
		albumId: string;
	};

	const { albumId } = $props<Props>();
	let albumName: string | null = $state(null);
	let albumDesc: string | null = $state(null);

	const layoutOptions: LayoutOptions = {
		targetRowHeight: 200,
		gap: 4
	};
	let containerWidth: number | null = $state(null);
	let assets: Asset[] = $state([]);
	const gridLayout: GridLayout | null = $derived.call(() => {
		if (!containerWidth) {
			return null;
		}
		return computeLayout(assets, containerWidth, layoutOptions);
	});
	$inspect(containerWidth);

	onMount(() => {
		fetchAlbumDetails();
	});

	async function fetchAlbumDetails() {
		const details = await api.getAlbumDetails({ params: { id: albumId } });
		assets = details.assets;
		albumName = details.name;
		albumDesc = details.description;
	}

	type LayoutOptions = {
		targetRowHeight: number;
		gap: number;
	};
	type GridLayout = {
		height: number;
		tiles: TileBox[];
	};
	function computeLayout(
		assets: Asset[],
		containerWidth: number,
		options: LayoutOptions
	): GridLayout {
		const assetSizes = assets.map((asset) => {
			return { width: asset.width, height: asset.height };
		});
		const layout = createJustifiedLayout(assetSizes, {
			targetRowHeight: options.targetRowHeight,
			containerWidth,
			boxSpacing: options.gap,
			containerPadding: 0
		});
		return {
			height: layout.containerHeight,
			tiles: layout.boxes
		};
	}
</script>

<div class="flex flex-col mx-60">
	<p class="text-6xl">{albumName}</p>
	<div
		bind:clientWidth={containerWidth}
		class="relative w-full"
		style="height: {gridLayout ? gridLayout.height : 0}px;"
	>
		{#if gridLayout}
			{#each gridLayout.tiles as tile, index (assets[index])}
				{@const asset = assets[index]}
				<img
					style="position: absolute; width: {tile.width}px; height: {tile.height}px; top: {tile.top}px; left: {tile.left}px;"
					src="/api/asset/thumbnail/{asset.id}/large/avif"
				/>
			{/each}
		{/if}
	</div>
</div>
