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
  import type { AssetWithSpe } from '@api/myrti';
  import {
    mdiProgressWrench,
    mdiPlayCircleOutline,
    mdiCheckCircle,
    mdiCheckCircleOutline,
    mdiCheckboxMarkedCircle,
    mdiCircleOutline,
  } from '@mdi/js';
  import { Layers } from '@lucide/svelte';
  import { fade } from 'svelte/transition';
  import { thumbHashToRGBA } from 'thumbhash';
  import { base91Decode } from '@lib/base91';
  import { onDestroy, untrack } from 'svelte';
  import { imageQueue } from '@lib/timeline-grid/image-fetch-queue';

  type GridTileProps = {
    href: string;
    asset: AssetWithSpe;
    box: TileBox;
    selectState: SelectState;
    showStackIcon: boolean | undefined;
    onSelectToggled: () => void;
    onAssetClick: () => void;
    imgElId: string;
    className: string | undefined;
  };
  let {
    href,
    asset,
    box,
    selectState,
    showStackIcon,
    onSelectToggled,
    onAssetClick,
    className,
    imgElId,
    imgElAction,
    viewportDistance,
  }: GridTileProps = $props();
  let isMouseOver = $state(false);
  const isSelected = $derived(selectState.state === 'select' && selectState.isSelected);
  let canvas: HTMLCanvasElement | null = $state(null);

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

  const thumbhash = $derived(
    asset.repFile.thumbhash ? base91Decode(asset.repFile.thumbhash) : null,
  );
  const mirrorImg = $derived(asset.repFile.mirrorCorrection);
  const rotateImg = $derived(asset.repFile.rotationCorrection);
  const rotateImgMod180 = $derived(asset.repFile.rotationCorrection % 180);
  const imgTop = $derived(rotateImgMod180 !== 0 ? (box.height - box.width) * 0.5 : 0);
  const imgLeft = $derived(rotateImgMod180 !== 0 ? (box.width - box.height) * 0.5 : 0);
  const imgHeight = $derived(rotateImgMod180 != 0 ? box.width : box.height);
  const imgWidth = $derived(rotateImgMod180 != 0 ? box.height : box.width);
  const isHoverable = $derived(selectState.state !== 'unclickable');

  let thumbHashVisible = $state(false);
  const thumbnailUrl = $derived(`/api/files/${asset.repFile.fileId}/thumbnail/large/avif`);
  let dataUrl: string | null = $state(null);

  $effect(() => {
    imageQueue.updatePriority(thumbnailUrl, viewportDistance);
  });

  $effect(() => {
    if (thumbnailUrl && viewportDistance < 400) {
      imageQueue
        .fetch(thumbnailUrl, viewportDistance)
        .then((url) => {
          dataUrl = url;
        })
        .catch(() => null);
    } else if (thumbnailUrl) {
      imageQueue.abort(thumbnailUrl);
      dataUrl = null;
    }
  });

  onDestroy(() => {
    imageQueue.abort(thumbnailUrl);
    dataUrl = null;
  });

  function thumbnailLoadOnce(node: HTMLImageElement) {
    function onLoad() {
      thumbHashVisible = false;
      node.removeEventListener('load', onLoad);
    }

    // Check if image was already cached before action mounted
    if (node.complete && node.naturalWidth > 0) {
      onLoad();
    } else {
      thumbHashVisible = true;
      node.addEventListener('load', onLoad);
    }

    return {
      destroy() {
        node.removeEventListener('load', onLoad);
      },
    };
  }

  $effect(() => {
    const ctx = canvas?.getContext('2d');
    const canvasTmp = document.createElement('canvas');
    const ctxTmp = canvasTmp.getContext('2d');
    if (!canvas || !ctx || !ctxTmp || !thumbhash) {
      return;
    }
    const { w, h, rgba } = thumbHashToRGBA(thumbhash);

    const rotation = asset.repFile.rotationCorrection;
    canvas.width = rotation % 180 == 0 ? w : h;
    canvas.height = rotation % 180 == 0 ? h : w;
    canvasTmp.width = w;
    canvasTmp.height = h;

    const pixels = new ImageData(new Uint8ClampedArray(rgba), w, h);
    ctxTmp.putImageData(pixels, 0, 0);

    ctx.translate(canvas.width / 2, canvas.height / 2);
    ctx.rotate((rotation * Math.PI) / 180);
    ctx.drawImage(canvasTmp, -w / 2, -h / 2);
  });
</script>

<a
  {href}
  id={'grid-a-' + asset.assetId}
  class={'absolute block group select-none outline-none' +
    className +
    ' ' +
    (isHoverable ? '' : 'cursor-default')}
  style="width: {box.width}px; height: {box.height}px; top: {box.top}px; left: {box.left}px;"
  onclick={(e) => {
    e.preventDefault();
    e.stopPropagation();
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
  {@attach imgElAction}
>
  <div class="h-full w-full bg-blue-100">
    <canvas
      bind:this={canvas}
      class="absolute"
      class:rounded-xl={isSelected}
      class:scale-[0.85]={isSelected}
      style:width="{box.width}px"
      style:height="{box.height}px"
      style:transform={mirrorImg === 'vertical'
        ? 'scaleY(-1)'
        : mirrorImg === 'horizontal'
          ? 'scaleX(-1)'
          : ''}
    ></canvas>
    {#if dataUrl}
      <img
        {@attach thumbnailLoadOnce}
        id={imgElId}
        src={dataUrl}
        class="absolute transition-opacity"
        class:rounded-xl={isSelected}
        class:scale-[0.85]={isSelected}
        width={imgWidth}
        height={imgHeight}
        style:opacity={thumbHashVisible ? '0' : '1'}
        style:top={imgTop + 'px'}
        style:left={imgLeft + 'px'}
        style:max-width="none"
        style:transform="rotate({rotateImg}deg) {mirrorImg === 'vertical'
          ? 'scaleY(-1)'
          : mirrorImg === 'horizontal'
            ? 'scaleX(-1)'
            : ''}"
      />
    {/if}

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
      <Layers
        class="absolute right-0 mr-1 mt-1 md:mr-2 md:mt-2 bg-black/30"
        size="24"
        color="white"
      />
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
