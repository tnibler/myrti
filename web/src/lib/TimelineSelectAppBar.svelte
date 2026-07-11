<script lang="ts">
  import { mdiClose } from '@mdi/js';
  import IconButton from './ui/IconButton.svelte';
  import {
    BookImage,
    EyeOff,
    ImageMinus,
    Images,
    Layers,
    LayersPlus,
    SquareStack,
  } from '@lucide/svelte';
  import { intl } from '@lib/i18next';

  type ButtonProp = {
    visible: boolean;
    onClick: () => void;
  };

  type Props = {
    numAssetsSelected: number;

    cancelSelect: ButtonProp;
    addToAlbum: ButtonProp;
    addToGroup: ButtonProp;
    removeFromGroup: ButtonProp;
    createStack: ButtonProp;
    addToStack: ButtonProp;
    deleteStack: ButtonProp;
    hide: ButtonProp;
  };
  let {
    numAssetsSelected,
    cancelSelect,
    addToAlbum,
    addToGroup,
    removeFromGroup,
    createStack,
    deleteStack,
    addToStack,
    hide,
  }: Props = $props();
</script>

<div
  class="w-full lg:h-[--appbar-height] basis-[--appbar-height]
  px-3 gap-4 shadow-md
  flex flex-row items-center justify-between
  "
>
  <div class="flex flex-row items-center">
    {#if cancelSelect.visible}
      <button
        class="p-3 hover:bg-gray-200 bg-transparent rounded-full transition-colors"
        onclick={cancelSelect.onClick}
      >
        <svg width="24" height="24" viewBox="0 0 24 24">
          <path d={mdiClose} fill="#000" />
        </svg>
      </button>
      <div class="font-medium">
        {intl('n_selected', { n: numAssetsSelected })}
      </div>
    {/if}
  </div>

  <div class="flex flex-row items-center gap-3 px-2">
    {#if addToGroup.visible}
      <IconButton onclick={addToGroup.onClick} text={intl('add_to_group')}><Images /></IconButton>
    {/if}
    {#if createStack.visible}
      <IconButton onclick={createStack.onClick} text={intl('create_stack')}><Layers /></IconButton>
    {/if}
    {#if removeFromGroup.visible}
      <IconButton onclick={removeFromGroup.onClick} text={intl('remove_from_group')}
        ><ImageMinus /></IconButton
      >
    {/if}
    {#if addToStack.visible}
      <IconButton onclick={addToStack.onClick} text={intl('add_to_stack')}
        ><LayersPlus /></IconButton
      >
    {/if}
    {#if deleteStack.visible}
      <IconButton onclick={deleteStack.onClick} text={intl('delete_stack')}
        ><SquareStack /></IconButton
      >
    {/if}
    {#if addToAlbum.visible}
      <IconButton onclick={addToAlbum.onClick} text={intl('add_to_album')}
        ><BookImage />
      </IconButton>
    {/if}
    {#if hide.visible}
      <IconButton onclick={hide.onClick} text={intl('hide')}><EyeOff /></IconButton>
    {/if}
  </div>
</div>
