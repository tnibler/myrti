<script lang="ts">
  import * as maplibregl from 'maplibre-gl';
  import { onDestroy } from 'svelte';

  let markerEl: HTMLElement;
  const { assetId, map, longLat }: { assetId } = $props();
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
  src="/api/assets/thumbnail/{assetId}/small/avif"
  class="rounded-lg"
  style="width: 100px;"
/>
