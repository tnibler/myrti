<script lang="ts">
  import type { Snippet } from 'svelte';
  import Slide, { type SlideProps } from './Slide.svelte';

  type SlideHolderProps = {
    xTransform: number;
    id: number;
    children: Snippet;
  };
  let { xTransform, id, children }: SlideHolderProps = $props();
  // let slideComponent: Slide | null = $state(null);
  // export function slideControls() {
  //   return slideComponent?.controls;
  // }
  const transformStr: string = $derived(`translate3d(${Math.round(xTransform)}px, 0px, 0px)`);
</script>

<div id="id-{id}" class="item" style="transform: {transformStr};">
  {@render children()}
  <!-- <Slide bind:this={slideComponent} {...slideProps} /> -->
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
