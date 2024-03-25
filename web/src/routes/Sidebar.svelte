<script context="module" lang="ts">
	export type SidebarEntry = 'timeline' | 'albums' | 'map';
</script>

<script lang="ts">
	import { BookImageIcon, ImagesIcon, MapIcon } from 'lucide-svelte';
	import SidebarItem from './SidebarItem.svelte';

	type Props = {
		activeEntry: SidebarEntry;
	};

	const { activeEntry } = $props<Props>();
</script>

<aside id="sidebar" class="h-dvh w-56 flex flex-col py-4">
	<a href="/" draggable="false"
		><SidebarItem title="Photos" isActive={activeEntry === 'timeline'}>
			<ImagesIcon />
		</SidebarItem>
	</a>
	<a href="/albums" draggable="false">
		<SidebarItem title="Albums" isActive={activeEntry === 'albums'}>
			<BookImageIcon />
		</SidebarItem>
	</a>
	<a href="/map" draggable="false">
		<SidebarItem title="Map" isActive={activeEntry === 'map'}>
			<MapIcon />
		</SidebarItem>
	</a>
</aside>
