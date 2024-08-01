<script lang="ts">
  import Button from '@lib/ui/Button.svelte';
  import IconButton from '@lib/ui/IconButton.svelte';
  import { mdiClose } from '@mdi/js';

  type Props = {
    mode: { mode: 'default' } | { mode: 'select'; numItemsSelected: number };
    onCancelSelectClicked: () => void;
    onRemoveFromAlbumClicked: () => void;
  };

  const { mode, onCancelSelectClicked, onRemoveFromAlbumClicked }: Props = $props();
</script>

<header
  class="w-full lg:h-[--appbar-height] basis-[--appbar-height]
  flex flex-ro items-center justify-between"
>
  {#if mode.mode === 'select'}
    <div class="flex flex-row items-center">
      <button
        class="p-3 hover:bg-gray-200 bg-transparent rounded-full transition-colors"
        onclick={() => {
          onCancelSelectClicked();
        }}
      >
        <svg width="24" height="24" viewBox="0 0 24 24">
          <path d={mdiClose} fill="#000" />
        </svg>
      </button>
      <div class="font-medium">
        {mode.numItemsSelected} selected
      </div>
    </div>
    <div class="flex flex-row items-center gap-3 px-2 self-end">
      <IconButton
        text={'Remove from album'}
        onclick={() => {
          onRemoveFromAlbumClicked();
        }}
        class=""
      />
    </div>
  {/if}
</header>
