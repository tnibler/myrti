<script lang="ts" context="module">
  export type TileBox = {
    width: number;
    height: number;
    top: number;
    left: number;
  };

  export type SelectState =
    | {
        state: 'unclickable';
      }
    | { state: 'default' }
    | { state: 'select'; isSelected: boolean };
</script>

<script lang="ts">
  import type { Asset } from '@api/myrti';
  import {
    mdiProgressWrench,
    mdiPlayCircleOutline,
    mdiCheckCircle,
    mdiCheckCircleOutline,
    mdiCheckboxMarkedCircle,
    mdiCircleOutline,
  } from '@mdi/js';
  import { LayersIcon } from 'lucide-svelte';
  import { fade } from 'svelte/transition';

  type GridTileProps = {
    asset: Asset;
    box: TileBox;
    selectState: SelectState;
    showStackIcon: boolean | undefined;
    onSelectToggled: () => void;
    onAssetClick: () => void;
    imgEl: HTMLImageElement;
    className: string | undefined;
  };
  let {
    asset,
    box,
    selectState,
    showStackIcon,
    onSelectToggled,
    onAssetClick,
    imgEl = $bindable(),
    className,
  }: GridTileProps = $props();
  let isMouseOver = $state(false);
  const isSelected = $derived(selectState.state === 'select' && selectState.isSelected);

  function onSelectButtonClick() {
    onSelectToggled();
  }

  function onTileClick() {
    if (selectState.state === 'select') {
      onSelectToggled();
    } else if (selectState.state === 'default') {
      onAssetClick();
    } else {
      // 'unclickable'
    }
  }

  const rotateImgMod180 = (asset.rotationCorrection ?? 0) % 180;
  const imgTop = $derived(rotateImgMod180 == -90 ? box.height : 0);
  const imgLeft = $derived(rotateImgMod180 == 90 ? box.width : 0);
  const imgTransformOrigin = $derived(rotateImgMod180 != 0 ? 'top left' : 'center');
  const imgHeight = $derived(rotateImgMod180 != 0 ? box.width : box.height);
  const imgWidth = $derived(rotateImgMod180 != 0 ? box.height : box.width);
  const isHoverable = $derived(selectState.state !== 'unclickable');
</script>

<a
  href=""
  class={'absolute group select-none ' + className + ' ' + (isHoverable ? '' : 'cursor-default')}
  style="width: {box.width}px; height: {box.height}px; top: {box.top}px; left: {box.left}px;"
  onclick={(e) => {
    e.preventDefault();
    onTileClick();
  }}
  onmouseenter={() => {
    if (selectState.state !== 'unclickable') {
      isMouseOver = true;
    }
  }}
  onmouseleave={() => {
    isMouseOver = false;
  }}
>
  <div class="h-full w-full bg-blue-100">
    <!-- svelte-ignore a11y_missing_attribute -->
    <img
      bind:this={imgEl}
      src="/api/assets/thumbnail/{asset.id}/large/avif"
      class="absolute bg-black transition-transform"
      class:rounded-xl={isSelected}
      class:scale-[0.85]={isSelected}
      width={imgWidth}
      height={imgHeight}
      style:transform-origin={imgTransformOrigin}
      style:top={imgTop + 'px'}
      style:left={imgLeft + 'px'}
      style:max-width="none"
      style:rotate={asset.rotationCorrection ? asset.rotationCorrection + 'deg' : null}
    />
    <div
      class={'absolute z-10 h-full w-full bg-gradient-to-b from-black/25 via-[transparent_25%] opacity-0 transition-opacity ' +
        (isHoverable ? 'group-hover:opacity-100' : '')}
      class:rounded-xl={isSelected}
      class:scale-[0.85]={isSelected}
    ></div>
    {#if asset.assetType === 'video'}
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
    {#if showStackIcon}
      <LayersIcon class="absolute right-0 mr-1 mt-1 md:mr-2 md:mt-2" size="24" color="white" />
    {/if}
    <div class="absolute z-20 h-full w-full">
      {#if selectState.state === 'select' || (selectState.state === 'default' && isMouseOver)}
        {@const icon = isSelected
          ? mdiCheckboxMarkedCircle
          : selectState.state === 'select'
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
