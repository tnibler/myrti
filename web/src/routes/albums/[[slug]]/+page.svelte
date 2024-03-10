<script lang="ts">
	import { page } from '$app/stores';
	import MainLayout from '$lib/MainLayout.svelte';
	import { api } from '$lib/apiclient';
	import { type Album } from '$lib/apitypes';
	import { onMount } from 'svelte';

	let albums: Album[] = $state([]);

	onMount(() => {
		fetchAlbums();
	});

	async function fetchAlbums() {
		albums = await api.getAllAlbums();
	}

	const urlPathSlug = $derived($page.params.slug);
	const showScreen: { isDetail: false } | { isDetail: true; albumId: string } = $derived(
		urlPathSlug ? { isDetail: true, albumId: urlPathSlug } : { isDetail: false }
	);
</script>

{#snippet content()}
	{#if showScreen.isDetail}
		detail {showScreen.albumId}
	{:else}
		<div class="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5 gap-6 p-6">
			{#each albums as album (album.id)}
				<a href="/albums/{album.id}">
					<div class="flex flex-col">
						<img class="flex-1 w-full aspect-square rounded-xl bg-gray-500" alt={album.name} />
						<p class="ml-1 mt-1 font-medium">{album.name}</p>
						<p class="ml-1 font-medium text-xs">{album.numAssets} element</p>
					</div>
				</a>
			{/each}
		</div>
	{/if}
{/snippet}

<MainLayout {content} activeSideBarEntry="albums" showAppBarOverride={false} />
