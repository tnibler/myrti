<script lang="ts" context="module">
	import type { TimelineSegment } from '$lib/apitypes';

	export type SegmentLayout = {
		segment: TimelineSegment;
		top: number;
		height: number;
		width: number;
		tiles: Tile[];
	};

	export type Tile = {
		width: number;
		height: number;
		top: number;
		left: number;
	};
</script>

<script lang="ts">
	let { layout }: { layout: SegmentLayout } = $props();
</script>

<div
	id="segment-{layout.segment.id}"
	class="segment"
	style="width: {layout.width}px; height: {layout.height}px; top: {layout.top}px; left: 0px;"
>
	{#each layout.tiles as box, assetIdx}
		<img
			src="/api/asset/thumbnail/{layout.segment.assets[assetIdx].id}/large/avif"
			class="tile"
			style="width: {box.width}px; height: {box.height}px; top: {box.top}px; left: {box.left}px;"
		/>
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
