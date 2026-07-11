<script lang="ts">
  // Taken from: https://github.com/MIERUNE/svelte-maplibre-gl/tree/main
  // Copyright svelte-maplibre-gl contributors
  // MIT License
  //
  // Permission is hereby granted, free of charge, to any person obtaining a copy
  // of this software and associated documentation files (the "Software"), to deal
  // in the Software without restriction, including without limitation the rights
  // to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
  // copies of the Software, and to permit persons to whom the Software is
  // furnished to do so, subject to the following conditions:
  //
  // The above copyright notice and this permission notice shall be included in all
  // copies or substantial portions of the Software.
  //
  // THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
  // IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
  // FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
  // AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
  // LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
  // OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
  // SOFTWARE.

  // https://maplibre.org/maplibre-gl-js/docs/API/classes/Marker/

  import { onDestroy, untrack, type Snippet } from 'svelte';
  import * as maplibregl from 'maplibre-gl';

  interface Props extends Omit<maplibregl.MarkerOptions, 'className'> {
    map: maplibregl.Map;
    lnglat: maplibregl.LngLatLike;
    class?: string;
    /** HTML content of the marker */
    content?: Snippet;
    children?: Snippet;
    // Events
    // https://maplibre.org/maplibre-gl-js/docs/API/classes/Marker/#events
    ondrag?: maplibregl.Listener;
    ondragstart?: maplibregl.Listener;
    ondragend?: maplibregl.Listener;
    onclick?: maplibregl.Listener;
  }

  let container = $state<HTMLElement | null>(null);

  let {
    lnglat = $bindable(),
    map,
    class: className = undefined,
    draggable,
    rotation,
    rotationAlignment,
    pitchAlignment,
    opacity,
    color,
    opacityWhenCovered,
    offset,
    subpixelPositioning,
    content,
    children,
    ondrag,
    ondragstart,
    ondragend,
    onclick,
    ...restOptions
  }: Props = $props();

  let marker: maplibregl.Marker | null = $state.raw(null);

  const defaultOffset = untrack(
    () => (content || restOptions.element ? [0, 0] : [0, -14]) as maplibregl.PointLike,
  );

  $effect(() => {
    if (marker) {
      return;
    }
    const options: maplibregl.MarkerOptions = {
      draggable,
      offset,
      opacity,
      className,
      opacityWhenCovered,
      rotation,
      color,
      rotationAlignment,
      pitchAlignment,
      subpixelPositioning,
      ...restOptions,
    };

    if (content) {
      if (!container) throw new Error('Marker container is not initialized');
      options.element = container;
    }

    marker = new maplibregl.Marker(options);

    marker.setLngLat($state.snapshot(lnglat) as maplibregl.LngLatLike).addTo(map);

    marker.on('drag', (e) => {
      if (marker) {
        lnglat = formatLngLat(lnglat, marker.getLngLat());
      }
      ondrag?.(e);
    });
  });

  let firstRun = true;

  $effect(() => resetEventListener(marker, 'dragstart', ondragstart));
  $effect(() => resetEventListener(marker, 'dragend', ondragend));
  $effect(() => resetEventListener(marker, 'click', onclick));

  $effect(() => {
    draggable;
    if (!firstRun) {
      marker?.setDraggable(draggable);
    }
  });

  $effect(() => {
    if (lnglat && !firstRun) {
      marker?.setLngLat(lnglat);
    }
  });

  $effect(() => {
    rotation;
    if (!firstRun) {
      marker?.setRotation(rotation);
    }
  });

  $effect(() => {
    offset;
    if (!firstRun) {
      marker?.setOffset(offset ?? defaultOffset);
    }
  });
  $effect(() => {
    opacity;
    opacityWhenCovered;
    if (!firstRun) {
      marker?.setOpacity(opacity, opacityWhenCovered);
    }
  });
  $effect(() => {
    rotationAlignment;
    if (!firstRun) {
      marker?.setRotationAlignment(rotationAlignment);
    }
  });
  $effect(() => {
    pitchAlignment;
    if (!firstRun) {
      marker?.setPitchAlignment(pitchAlignment);
    }
  });
  $effect(() => {
    subpixelPositioning;
    if (!firstRun) {
      marker?.setSubpixelPositioning(subpixelPositioning ?? false);
    }
  });

  let prevClassNames: string[] = [];
  $effect(() => {
    const next = (className ?? '').split(/\s+/).filter(Boolean);
    if (!marker || firstRun) {
      // Initial classes are added by the Marker constructor via
      // MarkerOptions.className; record them so later diffs are correct.
      prevClassNames = next;
      return;
    }
    const nextSet = new Set(next);
    for (const c of prevClassNames) {
      if (!nextSet.has(c)) marker.removeClassName(c);
    }
    const prevSet = new Set(prevClassNames);
    for (const c of next) {
      if (!prevSet.has(c)) marker.addClassName(c);
    }
    prevClassNames = next;
  });

  $effect(() => {
    firstRun = false;
  });

  onDestroy(() => {
    marker?.remove();
  });

  function formatLngLat(
    target: maplibregl.LngLatLike,
    lnglat: maplibregl.LngLat,
  ): maplibregl.LngLatLike {
    if (Array.isArray(target)) {
      return [lnglat.lng, lnglat.lat];
    } else if ('lon' in target) {
      return { lon: lnglat.lng, lat: lnglat.lat };
    } else {
      return { lng: lnglat.lng, lat: lnglat.lat };
    }
  }

  function resetEventListener(
    evented: maplibregl.Evented | null | undefined,
    type: string,
    listener: maplibregl.Listener | undefined,
  ) {
    if (listener) {
      evented?.on(type, listener);
    }
    const prevListener = listener;
    return () => {
      if (prevListener) {
        evented?.off(type, prevListener);
      }
    };
  }
</script>

{#if content}
  <div bind:this={container}>
    {@render content()}
  </div>
{/if}

{@render children?.()}
