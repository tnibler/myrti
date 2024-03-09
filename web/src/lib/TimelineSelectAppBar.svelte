<script lang="ts">
	import { mdiClose, mdiGroup, mdiImageAlbum, mdiEyeOffOutline } from '@mdi/js';

	type Props = {
		numAssetsSelected: number;
		onCancelSelectClicked: () => void;
		onCreateAlbumClicked: () => void;
		onHideClicked: () => void;
	};
	let { numAssetsSelected, onCancelSelectClicked, onCreateAlbumClicked, onHideClicked } =
		$props<Props>();
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
			{numAssetsSelected} selected
		</div>
	</div>

	<div class="flex flex-row items-center">
		<button
			class="p-3 hover:bg-gray-200 bg-transparent rounded-full transition-colors"
			onclick={() => {}}
		>
			<svg width="24" height="24" viewBox="0 0 24 24">
				<path d={mdiGroup} fill="#000" />
			</svg>
		</button>
		<button
			class="p-3 hover:bg-gray-200 bg-transparent rounded-full transition-colors"
			onclick={() => {
				onCreateAlbumClicked();
			}}
		>
			<svg width="24" height="24" viewBox="0 0 24 24">
				<path d={mdiImageAlbum} fill="#000" />
			</svg>
		</button>
		<button
			class="p-3 hover:bg-gray-200 bg-transparent rounded-full transition-colors"
			onclick={() => {}}
		>
			<svg width="24" height="24" viewBox="0 0 24 24">
				<path d={mdiEyeOffOutline} fill="#000" />
			</svg>
		</button>
	</div>
</div>
