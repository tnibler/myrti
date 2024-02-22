<script lang="ts" context="module">
	import type { TimelineSegment } from '$lib/apitypes';

	export type SegmentLayout = {
		segment: TimelineSegment;
		top: number;
		height: number;
		width: number;
		tiles: Tile[];
		headerTop: number;
	};

	export type Tile = {
		width: number;
		height: number;
		top: number;
		left: number;
	};
</script>

<script lang="ts">
	let {
		layout,
		assetBaseIndex,
		onAssetClick
	}: {
		layout: SegmentLayout;
		assetBaseIndex: number;
		onAssetClick: (assetIndex: number) => void;
	} = $props();
	const segmentTitle = $derived.call(() => {
		const segment = layout.segment.segment;
		if (segment.type === 'userGroup') {
			return segment.name;
		} else {
			const options = {
				weekday: 'long',
				year: 'numeric',
				month: 'long',
				day: 'numeric'
			};
			const date = new Date(segment.start);
			return date.toLocaleDateString('de-DE', options);
		}
	});

	/** thumbnail els addressed by their index _in this segment_, not the global asset index in the whole timeline */
	let imgEls: HTMLImageElement[] = $state([]);

	/**
	@param assetIndex index in this segment, not global asset index in timeline
	*/
	export function getThumbImgForAsset(assetIndex: number): HTMLImageElement {
		console.assert(assetIndex >= 0);
		if (assetIndex >= imgEls.length) {
			console.error('segment was asked for thumbnail with index higher than number of assets');
		}
		return imgEls[assetIndex];
	}
</script>

<div
	id="segment-{layout.segment.id}"
	class="segment"
	style="width: {layout.width}px; height: {layout.height}px; top: {layout.top}px; left: 0px;"
>
	<h2 style="left: 20px; position: absolute; top: {layout.headerTop}px;">
		{segmentTitle}
	</h2>
	{#each layout.tiles as box, assetIdx}
		<a
			href="#"
			onclick={(e) => {
				e.preventDefault();
				onAssetClick(assetBaseIndex + assetIdx);
			}}
		>
			<!-- svelte-ignore a11y-missing-attribute -->
			<img
				bind:this={imgEls[assetIdx]}
				src="/api/asset/thumbnail/{layout.segment.assets[assetIdx].id}/large/avif"
				class="tile"
				style="width: {box.width}px; height: {box.height}px; top: {box.top}px; left: {box.left}px;"
			/>
		</a>
	{/each}
</div>

<style>
	.segment {
		position: absolute;
		contain: layout;
	}

	.tile {
		position: absolute;
		background-color: green;
	}
</style>
