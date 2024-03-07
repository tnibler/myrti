<script lang="ts" context="module">
	import type { TimelineSegment } from '$lib/apitypes';
	import { mdiCheckCircle } from '@mdi/js';
	import type { TimelineGridStore } from '$lib/store/timeline.svelte';

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

<h2 style="left: 20px; position: relative;">
	{segmentTitle}
</h2>
<div
	id="segment-{layout.segment.segment.id}"
	style="position: relative; height: {layout.height}px;"
>
	{#each layout.tiles as box, assetIdx}
		<a
			href="#"
			onclick={(e) => {
				e.preventDefault();
				onAssetClick(assetBaseIndex + assetIdx);
			}}
		>
			<div>
				<!-- svelte-ignore a11y-missing-attribute -->
				<img
					bind:this={imgEls[assetIdx]}
					src="/api/asset/thumbnail/{layout.segment.assets[assetIdx].id}/large/avif"
					class="tile"
					style="width: {box.width}px; height: {box.height}px; top: {box.top}px; left: {box.left}px; z-index: 10;"
				/>
				<div
					style="position: absolute; width: {box.width}px; height: {box.height}px; top: {box.top}px; left: {box.left}px; z-index: 20;"
				>
					<button
						style="padding: 4px; position: absolute; focus-outline: none; outline: none; background: none; border: none; cursor:pointer;"
						role="checkbox"
						aria-checked="false"
						onclick={(e) => {
							e.stopPropagation();
						}}
					>
						<svg style="opacity: 0.6;" width="24" height="24" viewBox="0 0 24 24"
							><path d={mdiCheckCircle} fill="#fff" /></svg
						>
					</button>
				</div>
			</div>
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
