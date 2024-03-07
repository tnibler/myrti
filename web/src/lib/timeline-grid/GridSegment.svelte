<script lang="ts" context="module">
	import type { TimelineSegment } from '$lib/apitypes';
	import type { TimelineGridStore } from '$lib/store/timeline.svelte';
	import { mdiCheckCircle, mdiCheckCircleOutline } from '@mdi/js';

	export type SegmentLayout = {
		segment: TimelineSegment;
		top: number;
		height: number;
		width: number;
		tiles: TileBox[];
		headerTop: number;
	};

	export type TileBox = {
		width: number;
		height: number;
		top: number;
		left: number;
	};
</script>

<script lang="ts">
	import GridTile from './GridTile.svelte';

	type GridSegmentProps = {
		timeline: TimelineGridStore;
		inSelectionMode: boolean;
		layout: SegmentLayout;
		assetBaseIndex: number;
		onAssetClick: (assetIndex: number) => void;
	};
	let { timeline, inSelectionMode, layout, assetBaseIndex, onAssetClick } =
		$props<GridSegmentProps>();
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

<h2 style="left: 20px; position: relative;">
	{segmentTitle}
</h2>
<div
	id="segment-{layout.segment.segment.id}"
	style="position: relative; height: {layout.height}px;"
>
	{#each layout.tiles as box, indexInSegment}
		{@const assetIndex = assetBaseIndex + indexInSegment}
		<GridTile
			{assetIndex}
			{timeline}
			{inSelectionMode}
			asset={layout.segment.assets[indexInSegment]}
			onAssetClick={() => onAssetClick(assetIndex)}
			{box}
			bind:imgEl={imgEls[indexInSegment]}
		/>
	{/each}
</div>

<style>
	.segment {
		position: absolute;
		contain: layout;
	}
</style>
