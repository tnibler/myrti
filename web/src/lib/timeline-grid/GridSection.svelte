<script lang="ts">
	import type { TimelineSegment } from "@lib/apitypes";
	import type {
		DisplaySection,
		TimelineGridStore,
	} from "@lib/store/timeline.svelte";
	import GridSegment from "./GridSegment.svelte";
	import type { SegmentLayout } from "./GridSegment.svelte";
	import createJustifiedLayout from "justified-layout";

	type GridSectionProps = {
		timeline: TimelineGridStore;
		inSelectionMode: boolean;
		sectionIndex: number;
		containerWidth: number;
		registerElementWithIntersectObserver: (el: HTMLElement) => () => void;
		isIntersecting: boolean;
		onAssetClick: (index: number) => void;
	};

	let {
		timeline,
		inSelectionMode,
		sectionIndex,
		containerWidth,
		registerElementWithIntersectObserver,
		isIntersecting,
		onAssetClick,
	}: GridSectionProps = $props();
	let sectionDivEl: HTMLElement;
	const section: DisplaySection = $derived(timeline.sections[sectionIndex]);

	const segmentStartIndices: number[] | undefined = $derived(
		timeline.sections[sectionIndex].segments
			? computeSegmentStartIndices(
					section.assetStartIndex,
					timeline.sections[sectionIndex].segments,
				)
			: undefined,
	);
	let gridSegments: GridSegment[] = $state([]);

	let justifiedLayouts = $derived(
		section.segments ? computeSegmentLayouts(section.segments) : [],
	);
	/** height of the actual DOM element, which we propagate up to the layout logic to shift the other sections 
	around as the content is loaded lazily **/
	let sectionHeight = $state(0);

	$effect(() => {
		const unregisterIntersectObserver =
			registerElementWithIntersectObserver(sectionDivEl);
		return unregisterIntersectObserver;
	});

	$effect(() => {
		if (sectionHeight != 0 && sectionHeight != section.height) {
			timeline.setRealSectionHeight(sectionIndex, sectionHeight);
		}
	});

	export function getThumbImgForAsset(assetIndex: number): HTMLImageElement {
		const segmentIndex = segmentStartIndices.findLastIndex(
			(startIndex) => startIndex <= assetIndex,
		);
		if (segmentIndex < 0) {
			console.error(
				`section ${sectionIndex} was asked for thumbnail element for asset at index ${assetIndex} but no matching segment was found.`,
			);
			return undefined;
		}
		const gridSegment = gridSegments[segmentIndex];
		return gridSegment.getThumbImgForAsset(
			assetIndex - segmentStartIndices[segmentIndex],
		);
	}

	function computeSegmentLayouts(segments: TimelineSegment[]): SegmentLayout[] {
		const targetRowHeight = timeline.layoutConfig.targetRowHeight;
		// this is only a guess and doesn't matter, the segment will fit its contents
		// including the header
		const headerHeight = timeline.layoutConfig.headerHeight;
		const segmentMargin = timeline.layoutConfig.segmentMargin;
		let layouts: SegmentLayout[] = [];
		let nextSegmentYMin = segmentMargin + headerHeight;
		for (const segment of segments) {
			const assetSizes = segment.assets.map((asset) => {
				if (asset.rotationCorrection && asset.rotationCorrection % 180 != 0) {
					return {
						width: asset.height,
						height: asset.width,
					};
				} else {
					return {
						width: asset.width,
						height: asset.height,
					};
				}
			});
			const geometry = createJustifiedLayout(assetSizes, {
				targetRowHeight,
				containerWidth,
				boxSpacing: timeline.layoutConfig.boxSpacing,
			});
			const height = geometry.containerHeight;
			layouts.push({
				segment: segment,
				top: nextSegmentYMin,
				width: containerWidth,
				height,
				tiles: geometry.boxes,
				headerTop: -headerHeight,
			});
			nextSegmentYMin += height + segmentMargin + headerHeight;
		}
		return layouts;
	}

	function computeSegmentStartIndices(
		sectionBaseIndex: number,
		segments: TimelineSegment[],
	): number[] {
		if (segments.length == 0) {
			return [];
		} else if (segments.length == 1) {
			return [sectionBaseIndex];
		}
		let idxs: number[] = [sectionBaseIndex];
		for (let i = 1; i < segments.length; i += 1) {
			idxs.push(idxs[i - 1] + segments[i - 1].assets.length);
		}
		return idxs;
	}

	// if section is not visible, set the (estimated) size explicitly, otherwise let its size
	// fit the content.
	// This makes it easier to handle e.g., font size/text zoom in the header without breaking the layout
	// or recalculating stuff in js.
	const explicitSectionHeight = $derived(
		isIntersecting ? "" : `height: ${section.height}px;`,
	);
</script>

<div
	bind:this={sectionDivEl}
	bind:clientHeight={sectionHeight}
	class="grid-section"
	id="section-{sectionIndex}"
	style="width: {containerWidth}px;  top: {section.top}px; left: 0px; {explicitSectionHeight}"
>
	{#if isIntersecting && section.segments}
		{#each section.segments as segment, idx}
			<GridSegment
				{timeline}
				{inSelectionMode}
				bind:this={gridSegments[idx]}
				layout={justifiedLayouts[idx]}
				assetBaseIndex={segmentStartIndices[idx]}
				{onAssetClick}
			/>
		{/each}
	{/if}
</div>

<style>
	.grid-section {
		position: absolute;
		contain: layout;
	}
</style>
