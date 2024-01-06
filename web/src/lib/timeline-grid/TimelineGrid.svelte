<script lang="ts">
	import { api } from '$lib/apiclient';
	import { createTimeline } from '$lib/store/timeline.svelte';
	import GridSection from './GridSection.svelte';
	import 'photoswipe/photoswipe.css';

	import PhotoSwipeLightbox from 'photoswipe/lightbox';
	import PhotoSwipeVideoPlugin from '../../routes/photoswipe-shaka.esm';
	import type { Asset } from '$lib/apitypes';

	type SlideType = 'image' | 'video';
	type SlideData<Ty extends SlideType> = {
		type: Ty;
		assetId: string;
		index: number;
		width: number;
		height: number;
		src: string;
		thumbSrc: string;
		mpdManifestUrl: Ty extends 'video' ? string : never;
	};

	let windowScrollY: number = $state(0);
	let viewport = $state({ width: 0, height: 0 });

	const layoutConfig = {
		targetRowHeight: 180,
		sectionMargin: 20
	};
	const timeline = $state(createTimeline(layoutConfig, api));

	let sectionsIntersecting: boolean[] = $state([]);
	$effect(async () => {
		await timeline.initialize(viewport);
		sectionsIntersecting.fill(false, 0, timeline.sections.length);
		initPhotoswipe();
	});

	let lightbox: PhotoSwipeLightbox;
	function initPhotoswipe() {
		lightbox = new PhotoSwipeLightbox({
			//showHideAnimationType: 'none',
			pswpModule: () => import('photoswipe'),
			// preload: [1, 2]
			loop: false
		});
		const _videoPlugin = new PhotoSwipeVideoPlugin(lightbox, {});
		lightbox.addFilter('numItems', (numItems: unknown) => {
			return timeline.totalNumAssets;
		});
		lightbox.addFilter('itemData', (itemData: SlideData, index: number) => {
			const asset: Asset | undefined = timeline.getAssetAtIndex(index);
			if (!asset) {
				console.log('asset is undefined');
				return undefined;
			}
			if (asset.type === 'image') {
				return <SlideData<'image'>>{
					type: 'image',
					assetId: asset.id,
					index: index,
					width: asset.width,
					height: asset.height,
					src: '/api/asset/original/' + asset.id,
					thumbSrc: '/api/asset/thumbnail/' + asset.id + '/large/avif'
				};
			} else {
				console.assert(asset.type === 'video');
				return {
					type: 'video',
					src: '/api/asset/original/' + asset.id,
					assetId: asset.id,
					index: index,
					thumbSrc: '/api/asset/thumbnail/' + asset.id + '/large/avif',
					width: asset.width,
					height: asset.height,
					mpdManifestUrl: '/api/dash/' + asset.id + '/stream.mpd'
				};
			}
		});

		lightbox.addFilter('thumbEl', (thumbEl: HTMLElement, data: SlideData, _index: number) => {
			const el = document.querySelector('[data-img-id="' + data.assetId + '"] img');
			if (el) {
				return el;
			}
			return thumbEl;
		});
		lightbox.addFilter('placeholderSrc', (placeholderSrc: unknown, slide: SlideData) => {
			const el = <HTMLImageElement | undefined>(
				document.querySelector('[data-img-id="' + slide.assetId + '"] img')
			);
			if (el) {
				return el.src;
			}
			return placeholderSrc;
		});
		lightbox.init();
	}

	function onAssetClick(index: number) {
		lightbox.loadAndOpen(index);
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

<section id="grid" bind:clientWidth={viewport.width} bind:clientHeight={viewport.height}>
	{#each timeline.sections as section, idx}
		<GridSection
			{timeline}
			sectionIndex={idx}
			containerWidth={viewport.width}
			{registerElementWithIntersectObserver}
			isIntersecting={sectionsIntersecting[idx]}
			{onAssetClick}
		/>
	{/each}
</section>

<style>
	#grid {
		position: relative;
	}
</style>
