<script lang="ts">
  import { onMount } from 'svelte';
  import type { Album } from '@api/myrti';
  import { getAllAlbumsResponse } from '../api/myrti.zod';
  import { getAllAlbums } from '../api/myrti';

  let albums: Album[] = $state([]);

  onMount(() => {
    fetchAlbums();
  });

  async function fetchAlbums() {
    albums = getAllAlbumsResponse.parse((await getAllAlbums()).data);
  }
</script>

<div class="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5 gap-6 p-6">
  {#each albums as album (album.id)}
    <a href="/albums/{album.id}">
      <div class="flex flex-col">
        <img
          class="flex-1 w-full aspect-square rounded-xl bg-gray-500"
          alt={album.name}
          src="/api/albums/{album.id}/thumbnail/SIZE_UNUSED/avif"
        />
        <p class="ml-1 mt-1 font-medium">{album.name}</p>
        <p class="ml-1 font-medium text-xs">{album.numAssets} element</p>
      </div>
    </a>
  {/each}
</div>
