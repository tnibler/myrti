<script lang="ts">
	import { api } from '$lib/apiclient';
	import { createTimeline, type TimelineGridStore } from '$lib/store/timeline.svelte';
	import GridSection from './GridSection.svelte';
	import 'photoswipe/photoswipe.css';
	import Gallery from '$lib/swipey-gallery/Gallery.svelte';
	import type { ThumbnailBounds } from '$lib/swipey-gallery/thumbnail-bounds';
	import type { SlideData } from '$lib/swipey-gallery/slide-data';

	import type { Asset } from '$lib/apitypes';

	let windowScrollY: number = $state(0);
	let viewport = $state({ width: 0, height: 0 });
	let gallery: Gallery;
	let bodyWrapper: HTMLDivElement;
	let gridSections: GridSection[] = $state([]);

	const layoutConfig = {
		targetRowHeight: 120,
		headerHeight: 50,
		sectionMargin: 20,
		segmentMargin: 20
	};
	const timeline: TimelineGridStore = $state(createTimeline(layoutConfig, api));

	let sectionsIntersecting: boolean[] = $state([]);
	$effect(async () => {
		await timeline.initialize(viewport);
		sectionsIntersecting.fill(false, 0, timeline.sections.length);
	});

	async function getSlide(index: number): Promise<SlideData | null> {
		const asset: Asset | null = await timeline.getAssetAtIndex(index);
		if (!asset) {
			console.log('asset is null');
			return null;
		}
		if (asset.type === 'image') {
			return {
				type: 'image',
				size: {
					width: asset.width,
					height: asset.height
				},
				src: '/api/asset/original/' + asset.id,
				placeholderSrc: '/api/asset/thumbnail/' + asset.id + '/large/avif'
			};
		} else if (asset.type === 'video') {
			return {
				type: 'video',
				src: '/api/asset/original/' + asset.id,
				placeholderSrc: '/api/asset/thumbnail/' + asset.id + '/large/avif',
				size: {
					width: asset.width,
					height: asset.height
				},
				mpdManifestUrl: `/api/dash/${asset.id}/stream.mpd`
			};
		}
		console.error('TODO no asset');
		return null;
	}

	function getThumbnailBounds(assetIndex: number): ThumbnailBounds {
		const sectionIndex = timeline.sections.findLastIndex((section, idx) => {
			return section.assetStartIndex <= assetIndex;
		});
		if (sectionIndex < 0) {
			console.error(`did not find section containing asset at index ${assetIndex}`);
			return { rect: { x: 100, y: 100, width: 100, height: 100 } };
		}
		const imgEl = gridSections[sectionIndex].getThumbImgForAsset(assetIndex);
		if (!imgEl) {
			return { rect: { x: 100, y: 100, width: 100, height: 100 } };
		}
		return { rect: { x: imgEl.x, y: imgEl.y, width: imgEl.width, height: imgEl.height } };
	}

	function onAssetClick(index: number) {
		gallery.open(index);
	}

	const intersectionObserver = new IntersectionObserver(handleSectionIntersect, {
		rootMargin: '200px 0px'
	});

	function handleSectionIntersect(entries: IntersectionObserverEntry[]) {
		entries.forEach((entry) => {
			const sectionDiv = entry.target;
			const sectionIndex = parseInt(sectionDiv.id.substring(8)); // section-123
			sectionsIntersecting[sectionIndex] = entry.isIntersecting;
			if (entry.isIntersecting) {
				timeline?.loadSection(sectionIndex);
			} else {
				// nothing
			}
		});
	}

	function registerElementWithIntersectObserver(el: HTMLElement): () => void {
		intersectionObserver.observe(el);
		return () => {
			intersectionObserver.unobserve(el);
		};
	}
</script>

<svelte:window bind:scrollY={windowScrollY} />

<div class="body-wrapper" bind:this={bodyWrapper}>
	<section id="grid" bind:clientWidth={viewport.width} bind:clientHeight={viewport.height}>
		{#each timeline.sections as section, idx}
			<GridSection
				bind:this={gridSections[idx]}
				{timeline}
				sectionIndex={idx}
				containerWidth={viewport.width}
				{registerElementWithIntersectObserver}
				isIntersecting={sectionsIntersecting[idx]}
				{onAssetClick}
			/>
		{/each}
	</section>
</div>

<Gallery
	bind:this={gallery}
	numSlides={timeline.totalNumAssets}
	{getSlide}
	{getThumbnailBounds}
	{bodyWrapper}
/>

<style>
	#grid {
		position: relative;
	}

	.body-wrapper {
		padding: 0px;
		height: 100%;
		width: 100%;
		position: relative;
		overflow-y: scroll;
	}
</style>
