<script>
	import { mdiImage, mdiImageAlbum, mdiMap } from '@mdi/js';
	import SidebarItem from './SidebarItem.svelte';
</script>

<aside id="sidebar" class="h-dvh w-56 flex flex-col py-4">
	<a href="/" draggable="false"><SidebarItem title="Photos" iconSvg={mdiImage} isActive={true} /></a
	>
	<a href="/albums" draggable="false"
		><SidebarItem title="Albums" iconSvg={mdiImageAlbum} isActive={false} /></a
	>
	<a href="/map" draggable="false"><SidebarItem title="Map" iconSvg={mdiMap} isActive={false} /></a>
</aside>
