<script lang="ts">
	import type { TimelineSegment } from '$lib/apitypes';
	import type { DisplaySection } from '$lib/store/timeline.svelte';
	import GridSegment from './GridSegment.svelte';
	import type { SegmentLayout } from './GridSegment.svelte';
	import type TimelineGrid from './TimelineGrid.svelte';
	import createJustifiedLayout from 'justified-layout';

	type GridSectionProps = {
		timeline: TimelineGrid;
		sectionIndex: number;
		containerWidth: number;
		registerElementWithIntersectObserver: (el: HTMLElement) => () => void;
		isIntersecting: boolean;
	};
	let {
		timeline,
		sectionIndex,
		containerWidth,
		registerElementWithIntersectObserver,
		isIntersecting
	}: GridSectionProps = $props();
	let sectionDivEl: HTMLElement;
	const section: DisplaySection = $derived(timeline.sections[sectionIndex]);

	let { layouts: justifiedLayouts, sectionHeight } = $derived(
		section.segments ? computeSegmentLayouts(section.segments) : { layouts: [], sectionHeight: 0 }
	);

	$effect(() => {
		console.log('section ', sectionIndex, isIntersecting);
	});

	$effect(() => {
		const unregisterIntersectObserver = registerElementWithIntersectObserver(sectionDivEl);
		return unregisterIntersectObserver;
	});

	$effect.pre(() => {
		if (sectionHeight != 0 && sectionHeight != section.height) {
			const delta = timeline.setRealSectionHeight(sectionIndex, sectionHeight);
			// TODO scroll more sveltily or smth
			if (window.scrollY > section.top) {
				console.log('scroll delta', delta, 'section', sectionIndex);
				window.scrollBy(0, delta);
			}
		}
	});

	function computeSegmentLayouts(segments: TimelineSegment[]): {
		layouts: SegmentLayout[];
		sectionHeight: number;
	} {
		const targetRowHeight = timeline.layoutConfig.targetRowHeight;
		// if (!isIntersecting) {
		// 	return [];
		// }
		let layouts: SegmentLayout[] = [];
		const segmentMargin = 20;
		let nextSegmentYMin = segmentMargin;
		for (const segment of segments) {
			const assetSizes = segment.assets.map((asset) => {
				return {
					width: asset.width,
					height: asset.height
				};
			});
			const geometry = createJustifiedLayout(assetSizes, { targetRowHeight, containerWidth });
			const height = geometry.containerHeight;
			layouts.push({
				segment: segment,
				top: nextSegmentYMin,
				width: containerWidth,
				height,
				tiles: geometry.boxes
			});
			nextSegmentYMin += height + segmentMargin;
		}
		return { layouts, sectionHeight: segments.length > 0 ? nextSegmentYMin : 0 };
	}
</script>

<div
	bind:this={sectionDivEl}
	class="grid-section"
	id="section-{sectionIndex}"
	style="width: {containerWidth}px; height: {section.height}px; top: {section.top}px; left: 0px;"
>
	{#if isIntersecting && section.segments}
		{#each section.segments as segment, idx}
			<GridSegment layout={justifiedLayouts[idx]} />
		{/each}
	{/if}
</div>

<style>
	.grid-section {
		position: absolute;
		contain: layout;
	}
</style>
