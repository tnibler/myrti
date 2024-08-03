<script context="module" lang="ts">
  export type SidebarEntry = 'timeline' | 'albums' | 'map';
</script>

<script lang="ts">
  import { BookImageIcon, ImagesIcon, MapIcon } from 'lucide-svelte';
  import SidebarItem from './SidebarItem.svelte';
  import { intl } from '@lib/i18next';

  type Props = {
    activeEntry: SidebarEntry;
  };

  const { activeEntry }: Props = $props();
</script>

<aside id="sidebar" class="h-dvh w-56 flex flex-col py-4">
  <a href="/" draggable="false"
    ><SidebarItem title={intl('nav.photos')} isActive={activeEntry === 'timeline'}>
      <ImagesIcon />
    </SidebarItem>
  </a>
  <a href="/albums" draggable="false">
    <SidebarItem title={intl('nav.albums')} isActive={activeEntry === 'albums'}>
      <BookImageIcon />
    </SidebarItem>
  </a>
  <a href="/map" draggable="false">
    <SidebarItem title={intl('nav.map')} isActive={activeEntry === 'map'}>
      <MapIcon />
    </SidebarItem>
  </a>
</aside>
