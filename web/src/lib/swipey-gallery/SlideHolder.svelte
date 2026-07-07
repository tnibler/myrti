<script lang="ts">
  import Slide, { type SlideProps } from './Slide.svelte';

  type SlideHolderProps = {
    xTransform: number;
    id: number;
  } & SlideProps;
  let { xTransform, id, ...slideProps }: SlideHolderProps & SlideProps = $props();
  let slideComponent: Slide | null = $state(null);

  export function slideControls() {
    return slideComponent?.controls;
  }
  const transformStr: string = $derived(`translate3d(${Math.round(xTransform)}px, 0px, 0px)`);
</script>

<div id="id-{id}" class="item" style="transform: {transformStr};">
  <Slide bind:this={slideComponent} {...slideProps} />
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
