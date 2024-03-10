<script lang="ts">
	import { mdiPlus } from '@mdi/js';
	import Button from './ui/Button.svelte';

	type Props = {
		onSubmit: (formData: { albumName: string }) => Promise<void>;
	};
	let { onSubmit } = $props<Props>();

	let dialog: HTMLDialogElement | null = $state(null);
	let albumNameInput: HTMLInputElement | null = $state(null);

	let showCreateAlbumForm = $state(false);
	let createButtonVisible = $state(true);

	export function open() {
		showCreateAlbumForm = false;
		createButtonVisible = true;
		dialog?.showModal();
	}

	export function close() {
		dialog?.close();
	}

	async function onCreateClicked() {
		createButtonVisible = false;
		const albumName = albumNameInput?.value;
		if (albumName === null || albumName === undefined || albumName.trim() === '') {
			return;
		}
		await onSubmit({ albumName });
	}

	function onNewAlbumClicked() {
		showCreateAlbumForm = true;
	}
</script>

<dialog bind:this={dialog} class="w-1/3 h-1/2 bg-transparent backdrop:bg-black/50">
	<div class="w-full h-full flex flex-col rounded-xl bg-white">
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
					<input type="submit" hidden />
					<Button
						text="Create"
						primary
						class="self-end {createButtonVisible ? '' : 'display-none'}"
						onclick={() => onCreateClicked()}
					/>
				</div>
			</form>
		{:else}
			<div class="flex-1 py-6 flex flex-col">
				<button class="py-4 px-6 hover:bg-gray-200" onclick={() => onNewAlbumClicked()}>
					<div class="flex flex-row items-center justify-start gap-6">
						<svg width="24" height="24" viewBox="0 0 24 24">
							<path d={mdiPlus} fill="#000" />
						</svg>
						New Album
					</div>
				</button>
			</div>
		{/if}
	</div>
</dialog>
