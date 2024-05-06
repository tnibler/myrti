<script lang="ts">
	import { api } from '$lib/apiclient';
	import type { Asset, AlbumItem, AssetWithSpe } from '$lib/apitypes';
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

	type Section = { type: 'text'; text: string } | { type: 'asset'; assets: AssetWithSpe[] };

	const layoutOptions: LayoutOptions = {
		targetRowHeight: 200,
		gap: 4
	};
	let containerWidth: number | null = $state(null);
	let items: AlbumItem[] = $state([]);
	let sections: Section[] = $derived.by(() => {
		let sections: Section[] = [];
		for (const item of items) {
			if (item.albumItemType === 'text') {
				sections.push({ type: 'text', text: item.text });
				continue;
			}
			console.assert(item.albumItemType === 'asset');
			const lastSection = sections[sections.length - 1];
			if (!lastSection || lastSection.type != 'asset') {
				sections.push({ type: 'asset', assets: [item.asset] });
			} else if (lastSection.type === 'asset') {
				lastSection.assets.push(item.asset);
			}
		}
		return sections;
	});

	type SectionLayout =
		| {
				type: 'text';
				text: string;
		  }
		| {
				type: 'asset';
				layout: GridLayout;
				assets: AssetWithSpe[];
		  };
	const layouts: SectionLayout[] = $derived.by(() => {
		const cw = containerWidth;
		if (cw === null) {
			return [];
		}
		return sections.map((section) => {
			if (section.type === 'text') {
				return { type: 'text', text: section.text };
			}
			console.assert(section.type === 'asset');
			const layout = computeLayout(section.assets, cw, layoutOptions);
			return { type: 'asset', layout, assets: section.assets };
		});
	});
	$inspect(layouts);

	onMount(() => {
		fetchAlbumDetails();
	});

	async function fetchAlbumDetails() {
		const details = await api.getAlbumDetails({ params: { id: albumId } });
		items = details.items;
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
			if (asset.rotationCorrection && asset.rotationCorrection % 180 != 0) {
				return {
					width: asset.height,
					height: asset.width
				};
			} else {
				return {
					width: asset.width,
					height: asset.height
				};
			}
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
		const item = items[index];
		if (!item || item.albumItemType !== 'asset') {
			return null;
		}
		const asset = item.asset;
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
		<div bind:clientWidth={containerWidth} class="w-full">
			{#each layouts as layout}
				{#if layout.type === 'asset'}
					<div class="relative w-full" style="height: {layout.layout.height}px;">
						{#each layout.layout.tiles as tile, index (items[index])}
							{@const asset = layout.assets[index]}
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
					</div>
				{:else if layout.type === 'text'}
					<h3>{layout.text}</h3>
				{/if}
			{/each}
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
