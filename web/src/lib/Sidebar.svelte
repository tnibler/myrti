<script context="module" lang="ts">
  export type SidebarEntry = 'timeline' | 'albums' | 'map';
</script>

<script lang="ts">
  import { BookImage, Images, Map } from '@lucide/svelte';
  import SidebarItem from './SidebarItem.svelte';
  import { intl } from '@lib/i18next';

  type Props = {
    activeEntry: SidebarEntry;
  };

  const { activeEntry }: Props = $props();
  let isOpen = $state(false);

  let touchStartX = 0;
  let touchCurrentX = 0;

  function onTouchStart(e) {
    touchStartX = e.touches[0].clientX;
  }

  function onTouchMove(e) {
    touchCurrentX = e.touches[0].clientX;
  }

  function handleTouchEnd() {
    if (touchStartX - touchCurrentX > 50 && touchCurrentX !== 0) {
      isOpen = false;
    }
    touchStartX = 0;
    touchCurrentX = 0;
  }
</script>

<button
  type="button"
  class="text-heading bg-transparent box-border hover:bg-neutral-secondary-medium font-medium leading-5 rounded-base ms-3 mt-3 text-sm p-2 inline-flex sm:hidden fixed top-0 left-0 z-10"
  onclick={() => (isOpen = !isOpen)}
  aria-controls="default-sidebar"
  aria-expanded={isOpen}
>
  <span class="sr-only">Open sidebar</span>
  <svg
    class="w-6 h-6"
    aria-hidden="true"
    xmlns="http://www.w3.org/2000/svg"
    width="24"
    height="24"
    fill="none"
    viewBox="0 0 24 24"
  >
    <path
      stroke="currentColor"
      stroke-linecap="round"
      stroke-width="2"
      d="M5 7h14M5 12h14M5 17h10"
    />
  </svg>
</button>

{#if isOpen}
  <div
    class="fixed inset-0 bg-black/50 z-10 sm:hidden transition-opacity duration-300"
    onclick={() => (isOpen = false)}
  ></div>
{/if}

<aside
  id="default-sidebar"
  class="fixed sm:relative flex flex-col top-0 left-0 z-10 w-64 h-full bg-white shadow-xl sm:shadow-none transition-transform duration-300 ease-in-out
    {isOpen ? 'translate-x-0' : '-translate-x-full sm:translate-x-0'}"
  aria-label="Sidebar"
  ontouchstart={onTouchStart}
  ontouchmove={onTouchMove}
  ontouchend={handleTouchEnd}
>
  <div class="h-16 flex items-center px-4 sm:hidden">
    <button class="text-gray-500 ml-auto" onclick={() => (isOpen = false)}>✕</button>
  </div>
  <a href="/" draggable="false"
    ><SidebarItem title={intl('nav.photos')} isActive={activeEntry === 'timeline'}>
      <Images />
    </SidebarItem>
  </a>
  <a href="/albums" draggable="false">
    <SidebarItem title={intl('nav.albums')} isActive={activeEntry === 'albums'}>
      <BookImage />
    </SidebarItem>
  </a>
  <a href="/map" draggable="false">
    <SidebarItem title={intl('nav.map')} isActive={activeEntry === 'map'}>
      <Map />
    </SidebarItem>
  </a>
</aside>

{#if isOpen}
  <div class="fixed z-100 bg-black opacity-30 w-full"></div>
{/if}

<!-- <aside id="sidebar" class="h-dvh w-56 flex flex-col py-4"> -->
<!-- </aside> -->
