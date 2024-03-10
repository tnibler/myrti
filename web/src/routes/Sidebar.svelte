<script context="module" lang="ts">
	export type SidebarEntry = 'timeline' | 'albums' | 'map';
</script>

<script lang="ts">
	import { mdiImage, mdiImageAlbum, mdiMap } from '@mdi/js';
	import SidebarItem from './SidebarItem.svelte';

	type Props = {
		activeEntry: SidebarEntry;
	};

	const { activeEntry } = $props<Props>();
</script>

<aside id="sidebar" class="h-dvh w-56 flex flex-col py-4">
	<a href="/" draggable="false"
		><SidebarItem title="Photos" iconSvg={mdiImage} isActive={activeEntry === 'timeline'} /></a
	>
	<a href="/albums" draggable="false"
		><SidebarItem title="Albums" iconSvg={mdiImageAlbum} isActive={activeEntry === 'albums'} /></a
	>
	<a href="/map" draggable="false"
		><SidebarItem title="Map" iconSvg={mdiMap} isActive={activeEntry === 'map'} /></a
	>
</aside>
