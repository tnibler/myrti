<script lang="ts">
  import maplibregl from 'maplibre-gl';
  import 'maplibre-gl/dist/maplibre-gl.css';
  import { Protocol } from 'pmtiles';
  import { onMount } from 'svelte';
  import { layers, namedFlavor } from '@protomaps/basemaps';
  import AssetMarker from '@lib/map/AssetMarker.svelte';
  import Marker from '@lib/map/Marker.svelte';
  import * as R from 'remeda';

  let markers = $state({});
  let clusterMarkers = $state({});
  let map: maplibregl.Map | null = $state(null);

  onMount(() => {
    let protocol = new Protocol();
    maplibregl.addProtocol('pmtiles', protocol.tile);
    map = new maplibregl.Map({
      container: 'mapContainer', // container id
      style: {
        version: 8,
        glyphs: 'https://protomaps.github.io/basemaps-assets/fonts/{fontstack}/{range}.pbf',
        sprite: 'https://protomaps.github.io/basemaps-assets/sprites/v4/light',
        sources: {
          protomaps: {
            type: 'vector',
            url: `pmtiles:///static/map.pmtiles`,
            attribution:
              '<a href="https://protomaps.com">Protomaps</a> © <a href="https://openstreetmap.org">OpenStreetMap</a>',
          },
          assets: {
            type: 'geojson',
            data: '/api/map/assetPoints',
            cluster: true,
            clusterRadius: 30,
            clusterMaxZoom: 20, // Max zoom to cluster points on
          },
        },
        layers: layers('protomaps', namedFlavor('light'), { lang: 'en' }),
      },
      center: [0, 0], // starting position [lng, lat]
      zoom: 1, // starting zoom
    });
    map.on('load', () => {
      map.addLayer({
        id: 'clusters',
        type: 'circle',
        source: 'assets',
        filter: ['has', 'point_count'],
        paint: {
          // 'circle-color': '#00f',
          // 'circle-opacity': 0.6,
          // 'circle-radius': ['step', ['get', 'point_count'], 20, 100, 30, 750, 40],
          'circle-radius': 0,
        },
      });
      // map.addLayer({
      //   id: 'asset_circle',
      //   type: 'circle',
      //   source: 'assets',
      //   filter: ['!=', 'cluster', true],
      //   paint: {
      //     'circle-color': '#f00',
      //     'circle-opacity': 0.6,
      //     'circle-radius': 12,
      //   },
      // });
    });
    // objects for caching and keeping track of HTML marker objects (for performance)
    // const markers = {};
    // let markersOnScreen = {};

    function updateMarkers() {
      const features = map.querySourceFeatures('assets');

      const clusterSource = map.getSource('assets');
      const newClusterMarkers = {};
      (async () => {
        for (let i = 0; i < features.length; i++) {
          const feat = features[i];
          const props = features[i].properties;
          if (!props.cluster) {
            continue;
          }
          const leaves = (
            await clusterSource.getClusterLeaves(props.cluster_id, props.point_count, 0)
          ).map((f) => {
            return {
              assetId: f.properties.id,
              takenDate: f.properties.taken_date,
              coords: f.geometry.coordinates,
            };
          });
          const newest = R.sortBy(leaves, (a) => a.takenDate)[leaves.length - 1];
          newClusterMarkers[newest.assetId] = newest;
        }
      })().then(() => {
        clusterMarkers = newClusterMarkers;
      });

      const newMarkers = {};
      for (let i = 0; i < features.length; i++) {
        const coords = features[i].geometry.coordinates;
        const props = features[i].properties;
        if (props.cluster) {
          continue;
        } else {
          newMarkers[props.id] = { coords, ...props };
        }
      }
      markers = newMarkers;

      // for (const id of markersOnScreen) {
      //   if (!(id in newMarkers)) {
      //     delete markers[id];
      //   }
      // }
    }

    // after the GeoJSON data is loaded, update markers on the screen and do so on every map move/moveend
    map.on('data', (e) => {
      if (e.sourceId !== 'assets' || !e.isSourceLoaded) return;

      map.on('move', updateMarkers);
      map.on('moveend', updateMarkers);
      updateMarkers();
    });

    map.on('click', 'clusters', function (e) {
      const features = map.queryRenderedFeatures(e.point, { layers: ['clusters'] });
      const clusterId = features[0].properties.cluster_id;
      const point_count = features[0].properties.point_count;
      const clusterSource = map.getSource('assets');

      clusterSource.getClusterLeaves(clusterId, point_count, 0).then(console.log);
    });
    // getAllAssets().then(async (assets) => {
    //   for (const asset of assets.data) {
    //     const details = (await getAssetDetails(asset.id)).data.exiftoolOutput;
    //     if ('EXIF' in details && 'GPSLatitude' in details['EXIF']) {
    //       const lat = details['EXIF']['GPSLatitude'];
    //       const lon = details['EXIF']['GPSLongitude'];
    //       new Marker().setLngLat([lon, lat]).addTo(map);
    //     }
    //   }
    // });
  });
</script>

<div id="mapContainer" class="h-full"></div>

{#each Object.entries(markers) as [id, marker] (id)}
  <Marker {map} lnglat={marker.coords}>
    {#snippet content()}
      <img src="/api/assets/thumbnail/{id}/small/avif" class="rounded-lg" style="width: 100px;" />
    {/snippet}
  </Marker>
{/each}
{#each Object.entries(clusterMarkers) as [id, marker] (id)}
  <Marker {map} lnglat={marker.coords}>
    {#snippet content()}
      <img src="/api/assets/thumbnail/{id}/small/avif" class="rounded-lg" style="width: 100px;" />
    {/snippet}
  </Marker>
{/each}

<!-- <MapLibre -->
<!--   class="h-full" -->
<!--   style={{ -->
<!--     version: 8, -->
<!--     glyphs: 'https://protomaps.github.io/basemaps-assets/fonts/{fontstack}/{range}.pbf', -->
<!--     sprite: 'https://protomaps.github.io/basemaps-assets/sprites/v4/light', -->
<!--     sources: { -->
<!--       protomaps: { -->
<!--         type: 'vector', -->
<!--         url: `pmtiles://localhost:5173/map.pmtiles`, -->
<!--         attribution: -->
<!--           '<a href="https://protomaps.com">Protomaps</a> © <a href="https://openstreetmap.org">OpenStreetMap</a>', -->
<!--       }, -->
<!--       assets: { -->
<!--         type: 'geojson', -->
<!--         data: '/api/map/assetPoints', -->
<!--         cluster: true, -->
<!--         clusterRadius: 30, -->
<!--         clusterMaxZoom: 20, // Max zoom to cluster points on -->
<!--       }, -->
<!--     }, -->
<!--     layers: layers('protomaps', namedFlavor('light'), { lang: 'en' }), -->
<!--   }} -->
<!-- > -->
<!--   <VectorTileSource url="pmtiles://localhost:5173/map.pmtiles"></VectorTileSource> -->
<!-- </MapLibre> -->
