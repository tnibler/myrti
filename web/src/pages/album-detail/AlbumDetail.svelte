<script lang="ts">
  import type { AlbumItem, AlbumItemId, AssetWithSpe } from '@api/myrti';
  import Gallery from '@lib/swipey-gallery/Gallery.svelte';
  import { type SlideData, slideForAsset } from '@lib/swipey-gallery/slide-data';
  import type { ThumbnailBounds } from '@lib/swipey-gallery/thumbnail-bounds';
  import type { TileBox } from '@lib/ui/GridTile.svelte';
  import GridTile from '@lib/ui/GridTile.svelte';
  import createJustifiedLayout from 'justified-layout';
  import { onMount } from 'svelte';
  import { SvelteSet } from 'svelte/reactivity';
  import AppBar from './AppBar.svelte';
  import * as R from 'remeda';
  import { deleteAlbumItems, getAlbumDetails } from '../../api/myrti';
  import { getAlbumDetailsResponse } from '../../api/myrti.zod';

  type Props = {
    albumId: string;
  };

  const { albumId }: Props = $props();
  let albumName: string | null = $state(null);
  let albumDesc: string | null = $state(null);

  type Section =
    | {
        type: 'text';
        item: { itemType: 'text' } & AlbumItem;
      }
    | {
        type: 'assets';
        items: ({ itemType: 'asset' } & AlbumItem)[];
      };

  const layoutOptions: LayoutOptions = {
    targetRowHeight: 200,
    gap: 4,
  };
  let containerWidth: number | null = $state(null);
  let items: AlbumItem[] = $state([]);
  let sections: Section[] = $derived.by(() => {
    let sections: Section[] = [];
    for (const item of items) {
      if (item.itemType === 'text') {
        sections.push({ type: 'text', item });
        continue;
      }
      console.assert(item.itemType === 'asset');
      const lastSection = sections[sections.length - 1];
      if (!lastSection || lastSection.type != 'assets') {
        sections.push({ type: 'assets', items: [item] });
      } else if (lastSection.type === 'assets') {
        lastSection.items.push(item);
      }
    }
    return sections;
  });
  const assets: AssetWithSpe[] = $derived.by(() => {
    return items.filter((item) => item.itemType === 'asset').map((item) => item.asset);
  });

  type SectionLayout =
    | {
        type: 'text';
        item: AlbumItem & { itemType: 'text' };
      }
    | {
        type: 'assets';
        height: number;
        tiles: { box: TileBox; item: AlbumItem & { itemType: 'asset' }; assetIndex: number }[];
      };
  const layouts: SectionLayout[] = $derived.by(() => {
    const cw = containerWidth;
    if (cw === null) {
      return [];
    }
    const l: SectionLayout[] = [];
    let firstAssetIndex = 0;
    for (const section of sections) {
      if (section.type === 'text') {
        l.push({ type: 'text', item: section.item });
      } else if (section.type === 'assets') {
        const layout = computeLayout(section.items, cw, firstAssetIndex, layoutOptions);
        l.push({ type: 'assets', tiles: layout.tiles, height: layout.height });
        firstAssetIndex += section.items.length;
      }
    }
    return l;
  });

  onMount(() => {
    fetchAlbumDetails();
  });

  async function fetchAlbumDetails() {
    const details = getAlbumDetailsResponse.parse((await getAlbumDetails(albumId)).data);
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
    tiles: { box: TileBox; item: AlbumItem & { itemType: 'asset' }; assetIndex: number }[];
  };
  function computeLayout(
    items: (AlbumItem & { itemType: 'asset' })[],
    containerWidth: number,
    firstAssetIndex: number,
    options: LayoutOptions,
  ): GridLayout {
    const assetSizes = items.map((item) => {
      const asset = item.asset;
      if (asset.rotationCorrection && asset.rotationCorrection % 180 != 0) {
        return {
          width: asset.height,
          height: asset.width,
        };
      } else {
        return {
          width: asset.width,
          height: asset.height,
        };
      }
    });
    const layout = createJustifiedLayout(assetSizes, {
      targetRowHeight: options.targetRowHeight,
      containerWidth,
      boxSpacing: options.gap,
      containerPadding: 0,
    });

    return {
      height: layout.containerHeight,
      tiles: R.zip(layout.boxes, items).map(([box, item], idx) => {
        return { box, item, assetIndex: firstAssetIndex + idx };
      }),
    };
  }

  let scrollContainer: HTMLElement | null = $state(null);
  let gallery: Gallery;
  /** maps asset index to thumbnail image element */
  let thumbnailImgEls: Record<number, HTMLImageElement> = $state({});
  const selectedItemIds: Set<AlbumItemId> = $state(new SvelteSet());
  const inSelectMode: boolean = $derived(selectedItemIds.size > 0);

  async function getSlide(index: number): Promise<SlideData | null> {
    return slideForAsset(assets[index]);
  }

  function getThumbnailBounds(assetIndex: number): ThumbnailBounds {
    const imgEl = thumbnailImgEls[assetIndex];
    if (!imgEl) {
      return { rect: { x: 0, y: 0, width: 0, height: 0 } };
    }
    return {
      rect: {
        x: imgEl.x,
        y: imgEl.y,
        width: imgEl.width,
        height: imgEl.height,
      },
    };
  }

  function onAssetClick(index: number) {
    gallery.open(index);
  }

  function toggleSelected(itemId: AlbumItemId) {
    const isSelected = selectedItemIds.has(itemId);
    if (isSelected) {
      selectedItemIds.delete(itemId);
    } else {
      selectedItemIds.add(itemId);
    }
  }

  async function onRemoveFromAlbumClicked() {
    await deleteAlbumItems(albumId, { itemIds: Array.from(selectedItemIds) });

    items = items.filter((item) => !selectedItemIds.has(item.itemId));
    selectedItemIds.clear();
  }

  function onCancelSelectClicked() {
    selectedItemIds.clear();
  }
</script>

<div class="flex flex-col h-dvh">
  <AppBar
    mode={inSelectMode
      ? { mode: 'select', numItemsSelected: selectedItemIds.size }
      : { mode: 'default' }}
    {onRemoveFromAlbumClicked}
    {onCancelSelectClicked}
  />
  <div class="h-dvh w-full relative overflow-y-hidden">
    <div bind:this={scrollContainer} class="h-full flex flex-col overflow-y-scroll">
      <div class="w-2/3 self-center py-12">
        <p class="text-6xl">{albumName}</p>
        <div bind:clientWidth={containerWidth} class="w-full mt-8">
          {#each layouts as layout}
            {#if layout.type === 'assets'}
              <div class="relative w-full" style="height: {layout.height}px;">
                {#each layout.tiles as tile (tile.item.itemId)}
                  {@const asset = tile.item.asset}
                  <GridTile
                    {asset}
                    box={tile.box}
                    onAssetClick={() => {
                      onAssetClick(tile.assetIndex);
                    }}
                    onSelectToggled={() => {
                      toggleSelected(tile.item.itemId);
                    }}
                    selectState={inSelectMode //
                      ? { state: 'select', isSelected: selectedItemIds.has(tile.item.itemId) }
                      : { state: 'default' }}
                    bind:imgEl={thumbnailImgEls[tile.assetIndex]}
                    className={'timeline-item-transition'}
                  />
                {/each}
              </div>
            {:else if layout.type === 'text'}
              <h3>{layout.item.text}</h3>
            {/if}
          {/each}
        </div>
      </div>
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
