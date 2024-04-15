<script lang="ts">
	import { api } from '$lib/apiclient';
	import type { Asset } from '$lib/apitypes';
	import Gallery from '$lib/swipey-gallery/Gallery.svelte';
	import { type SlideData, slideForAsset } from '$lib/swipey-gallery/slide-data';
	import type { ThumbnailBounds } from '$lib/swipey-gallery/thumbnail-bounds';
	import type { TileBox } from '$lib/timeline-grid/GridSegment.svelte';
	import GridTile from '$lib/ui/GridTile.svelte';
	import createJustifiedLayout from 'justified-layout';
	import { onMount } from 'svelte';

	type Props = {
		albumId: string;
	};

	const { albumId }: Props = $props();
	let albumName: string | null = $state(null);
	let albumDesc: string | null = $state(null);

	const layoutOptions: LayoutOptions = {
		targetRowHeight: 200,
		gap: 4
	};
	let containerWidth: number | null = $state(null);
	let assets: Asset[] = $state([]);
	const gridLayout: GridLayout | null = $derived.by(() => {
		if (!containerWidth) {
			return null;
		}
		return computeLayout(assets, containerWidth, layoutOptions);
	});

	onMount(() => {
		fetchAlbumDetails();
	});

	async function fetchAlbumDetails() {
		const details = await api.getAlbumDetails({ params: { id: albumId } });
		assets = details.assets;
		albumName = details.name ?? null;
		albumDesc = details.description ?? null;
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

	let scrollContainer: HTMLElement | null = $state(null);
	let gallery: Gallery;
	let thumbnailImgEls: HTMLImageElement[] = $state([]);

	async function getSlide(index: number): Promise<SlideData | null> {
		const asset = assets[index];
		if (!asset) {
			console.log('asset is null');
			return null;
		}
		return slideForAsset(asset);
	}

	function getThumbnailBounds(assetIndex: number): ThumbnailBounds {
		const imgEl = thumbnailImgEls[assetIndex];
		if (!imgEl) {
			return { rect: { x: 0, y: 0, width: 0, height: 0 } };
		}
		return { rect: { x: imgEl.x, y: imgEl.y, width: imgEl.width, height: imgEl.height } };
	}

	function onAssetClick(index: number) {
		gallery.open(index);
	}
</script>

<div class="h-dvh w-full relative overflow-y-hidden">
	<div bind:this={scrollContainer} class="h-full flex flex-col px-10 overflow-y-scroll">
		<p class="text-6xl">{albumName}</p>
		<div
			bind:clientWidth={containerWidth}
			class="relative w-full"
			style="height: {gridLayout ? gridLayout.height : 0}px;"
		>
			{#if gridLayout}
				{#each gridLayout.tiles as tile, index (assets[index])}
					{@const asset = assets[index]}
					<GridTile
						{asset}
						box={tile}
						onAssetClick={() => {
							onAssetClick(index);
						}}
						onSelectToggled={() => {}}
						selectState={{ inSelectMode: false }}
						bind:imgEl={thumbnailImgEls[index]}
					/>
				{/each}
			{/if}
		</div>
	</div>
</div>

<Gallery
	bind:this={gallery}
	scrollWrapper={scrollContainer}
	numSlides={assets.length}
	{getSlide}
	{getThumbnailBounds}
/>
