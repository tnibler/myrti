<script lang="ts">
  import { mdiPlus } from '@mdi/js';
  import Button from './ui/Button.svelte';
  import { onMount } from 'svelte';
  import { api } from './apiclient';

  type Submit =
    | {
        action: 'createNew';
        albumName: string;
      }
    | {
        action: 'addTo';
        albumId: string;
      };

  type Props = {
    onSubmit: (formData: Submit) => Promise<void>;
  };
  let { onSubmit }: Props = $props();

  let dialog: HTMLDialogElement | null = $state(null);
  let albumNameInput: HTMLInputElement | null = $state(null);

  let albums: Album[] = $state([]);
  let showCreateAlbumForm = $state(false);
  let createButtonVisible = $state(true);

  export function open() {
    api.getAllAlbums().then((result) => {
      albums = result;
    });
    showCreateAlbumForm = false;
    createButtonVisible = true;
    dialog?.showModal();
  }

  export function close() {
    dialog?.close();
  }

  async function onCreateClicked(e: SubmitEvent) {
    e.preventDefault();
    createButtonVisible = false;
    const albumName = albumNameInput?.value;
    if (albumName === null || albumName === undefined || albumName.trim() === '') {
      return;
    }
    await onSubmit({ action: 'createNew', albumName });
  }

  async function onAlbumClicked(albumId: string) {
    await onSubmit({ action: 'addTo', albumId });
  }

  function onNewAlbumClicked() {
    showCreateAlbumForm = true;
  }
</script>

<dialog bind:this={dialog} class="w-1/3 h-1/2 bg-transparent backdrop:bg-black/50">
  <div class="w-full h-full flex flex-col rounded-xl bg-white overflow-hidden">
    <div
      class="flex flex-row justify-between items-baseline px-5 py-5 border-solid border-gray-200 border-b"
    >
      <p class="font-medium text-xl">
        {showCreateAlbumForm ? 'Create new album' : 'Add to album'}
      </p>
      <Button text="Close" onclick={() => dialog!.close()} />
    </div>
    {#if showCreateAlbumForm}
      <form onsubmit={onCreateClicked}>
        <div class="flex-1 py-4 px-6 flex flex-col justify-between">
          <input placeholder="Title" class="font-medium text-lg" bind:this={albumNameInput} />
          <Button
            text="Create"
            primary
            class="self-end {createButtonVisible ? '' : 'display-none'}"
            onclick={(e) => onCreateClicked(e)}
          />
        </div>
      </form>
    {:else}
      <div class="flex-1 py-6 flex flex-col overflow-y-scroll">
        <button class="py-4 px-6 hover:bg-gray-200" onclick={() => onNewAlbumClicked()}>
          <div class="flex flex-row items-center justify-start gap-6">
            <div class="w-16 aspect-square flex justify-around items-center">
              <svg width="24" height="24" viewBox="0 0 24 24">
                <path d={mdiPlus} fill="#000" />
              </svg>
            </div>
            New Album
          </div>
        </button>
        {#each albums as album (album.id)}
          <button class="py-4 px-6 hover:bg-gray-200" onclick={() => onAlbumClicked(album.id)}>
            <div class="flex flex-row items-center justify-start gap-6">
              <!-- svelte-ignore a11y-missing-attribute -->
              <img class="w-16 aspect-square rounded-sm bg-gray-400" />
              {album.name}
            </div>
          </button>
        {/each}
      </div>
    {/if}
  </div>
</dialog>
