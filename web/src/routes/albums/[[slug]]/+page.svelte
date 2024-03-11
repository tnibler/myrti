<script lang="ts">
	import { page } from '$app/stores';
	import MainLayout from '$lib/MainLayout.svelte';
	import { api } from '$lib/apiclient';
	import { type Album } from '$lib/apitypes';
	import { onMount } from 'svelte';
	import AlbumDetail from './AlbumDetail.svelte';
	import ListAlbums from './ListAlbums.svelte';

	const urlPathSlug = $derived($page.params.slug);
	const showScreen: { isDetail: false } | { isDetail: true; albumId: string } = $derived(
		urlPathSlug ? { isDetail: true, albumId: urlPathSlug } : { isDetail: false }
	);
</script>

{#if showScreen.isDetail}
	<AlbumDetail albumId={showScreen.albumId} />
{:else}
	{#snippet content()}
		<ListAlbums />
	{/snippet}
	<MainLayout {content} activeSideBarEntry="albums" showAppBarOverride={false} />
{/if}
