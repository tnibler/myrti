<script lang="ts">
	type Props = {
		numAssetsSelected: number;
		onCancelSelectClicked: () => void;
		onCreateAlbumClicked: () => void;
		onHideClicked: () => void;
	};
	let { numAssetsSelected, onCancelSelectClicked, onCreateAlbumClicked, onHideClicked } =
		$props<Props>();
</script>

<div class="w-full lg:h-[--appbar-height] basis-[--appbar-height] bg-green-500">
	<button
		onclick={() => {
			onCancelSelectClicked();
		}}>Cancel</button
	>
	{numAssetsSelected} selected
</div>
