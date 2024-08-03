<script lang="ts">
  import { mdiClose } from '@mdi/js';
  import IconButton from './ui/IconButton.svelte';
  import { BookImageIcon, EyeOffIcon, ImagesIcon } from 'lucide-svelte';
  import { intl } from '@lib/i18next';

  type Props = {
    numAssetsSelected: number;
    onCancelSelectClicked: () => void;
    onAddToAlbumClicked: () => void;
    onAddToGroupClicked: () => void;
    onHideClicked: () => void;
  };
  let {
    numAssetsSelected,
    onCancelSelectClicked,
    onAddToAlbumClicked,
    onAddToGroupClicked,
    onHideClicked,
  }: Props = $props();
</script>

<div
  class="w-full lg:h-[--appbar-height] basis-[--appbar-height]
  px-3 gap-4 shadow-md
  flex flex-row items-center justify-between
  "
>
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
      {intl('n_selected', { n: numAssetsSelected })}
    </div>
  </div>

  <div class="flex flex-row items-center gap-3 px-2">
    <IconButton
      onclick={() => {
        onAddToGroupClicked();
      }}
      text={intl('add_to_group')}><ImagesIcon /></IconButton
    >
    <IconButton
      onclick={() => {
        onAddToAlbumClicked();
      }}
      text={intl('add_to_album')}
      ><BookImageIcon />
    </IconButton>
    <IconButton
      onclick={() => {
        onHideClicked();
      }}
      text={intl('hide')}
      ><EyeOffIcon />
    </IconButton>
  </div>
</div>
