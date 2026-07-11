<script lang="ts">
  import * as maplibregl from 'maplibre-gl';
  import * as R from 'remeda';
  import { onDestroy } from 'svelte';

  let markerEl: HTMLElement;
  const { assets, map, longLat }: { assets: { id: string; takenDate: number }[] } = $props();
  const displayAsset = $derived(
    R.pipe(
      assets,
      R.sortBy((a) => a.takenDate),
    ).at(-1).id,
  );
  let marker: maplibregl.Marker | null = $state.raw(null);

  $effect(() => {
    if (marker) {
      return;
    }
    marker = new maplibregl.Marker({ element: markerEl });
    marker.setLngLat($state.snapshot(longLat)).addTo(map);
  });

  onDestroy(() => {
    marker?.remove();
  });
</script>

<img
  bind:this={markerEl}
  src="/api/assets/{displayAsset}/thumbnail/small/avif"
  class="rounded-lg"
  style="width: 100px;"
/>
