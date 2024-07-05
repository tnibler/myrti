<script lang="ts">
  import Button from './ui/Button.svelte';

  type Submit = {
    groupName: string;
  };

  type Props = {
    onSubmit: (formData: Submit) => Promise<void>;
  };
  let { onSubmit }: Props = $props();

  let dialog: HTMLDialogElement | null = $state(null);
  let albumNameInput: HTMLInputElement | null = $state(null);

  export function open() {
    dialog?.showModal();
  }

  export function close() {
    dialog?.close();
  }

  async function onCreateClicked(e: SubmitEvent) {
    e.preventDefault();
    const groupName = albumNameInput?.value.trim();
    if (!groupName || groupName === '') {
      return;
    }
    await onSubmit({ groupName });
  }
</script>

<dialog bind:this={dialog} class="w-1/3 h-1/2 bg-transparent backdrop:bg-black/50">
  <div class="w-full h-full flex flex-col rounded-xl bg-white overflow-hidden">
    <div
      class="flex flex-row justify-between items-baseline px-5 py-5 border-solid border-gray-200 border-b"
    >
      <p class="font-medium text-xl">Create new group</p>
      <Button text="Close" onclick={() => dialog!.close()} />
    </div>
    <form onsubmit={onCreateClicked}>
      <div class="flex-1 py-4 px-6 flex flex-col justify-between">
        <input placeholder="Title" class="font-medium text-lg" bind:this={albumNameInput} />
        <Button text="Create" primary class="self-end" onclick={(e) => onCreateClicked(e)} />
      </div>
    </form>
  </div>
</dialog>
