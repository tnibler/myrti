<script lang="ts">
  	import Main from './pages/Main.svelte'
	import ListAlbums from './pages/ListAlbums.svelte';
    import { path, resolve, params } from 'elegua';
    import MainLayout from '@lib/MainLayout.svelte';
    import AlbumDetail from './pages/AlbumDetail.svelte';
</script>




{#if $path === '/'}
  <Main/>
{:else if $path === '/albums'}
  {#snippet listAlbums()}
  	<ListAlbums/>
  {/snippet}
  <MainLayout
	  content={listAlbums}
	  showAppBarOverride={false}
	  activeSideBarEntry="albums"
  />
{:else if resolve($path, '/albums/:id')}
  <AlbumDetail albumId={$params['id']}/>
{/if}
