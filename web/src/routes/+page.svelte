<script lang="ts">
	// import Gallery from 'svelte-gallery';
	import Gallery from '$lib/Gallery';
	import type { Image } from '$lib/Gallery/Gallery.svelte';
	import { onMount } from 'svelte';
	import 'photoswipe/photoswipe.css';
	import PhotoSwipeLightbox from 'photoswipe/lightbox';
	import PhotoSwipeVideoPlugin from './photoswipe-shaka.esm';
	import { createApiClient, api } from '$lib/apiclient';
	import dayjs, { Dayjs } from 'dayjs';
	type AssetType = 'image' | 'video';

	type Asset = {
		id: string;
		assetRootId: string;
		pathInRoot: string;
		type: AssetType;
		width: number;
		height: number;
		addedAt: Dayjs;
		takenDate: Dayjs;
	};

	type TimelineGroupType =
		| {
				day: Dayjs;
		  }
		| { title: string; start: Dayjs; end: Dayjs };

	type TimelineGroup = {
		type: TimelineGroupType;
		assets: Asset[];
	};

	type TimelineChunk = {
		lastAssetId: string;
		changedSinceLastFetch: boolean;
		groups: TimelineGroup[];
	};

	let imgs: Image[] = [];
	let newImgs: Image[] = [];
	$: imgs = [...imgs, ...newImgs];
	let lastStartId: string | null = null;
	let index = 0;
	async function fetchMore() {
		api.get('/asset/:id');
		let startId = lastStartId ? 'lastAssetId=' + lastStartId.toString() + '&' : '';
		let res = await fetch('/api/asset/timeline?' + startId + 'maxCount=4&lastFetch=null');
		let json: TimelineChunk = await res.json();
		if (json.groups.length == 0) {
			return;
		}
		let lastGroup = json['groups'].at(-1);
		lastStartId = lastGroup['assets'].at(-1).id;
		let toAdd: Image[] = [];
		json['groups'].forEach((group: TimelineGroup) =>
			group['assets']
				//.filter((asset) => asset['type'] == 'image')
				.forEach((asset: Asset) => {
					toAdd.push({
						type: asset.type,
						index: index,
						thumbSrc: '/api/asset/thumbnail/' + asset.id + '/large/avif',
						src: '/api/asset/original/' + asset.id,
						width: asset.width,
						height: asset.height,
						mpdManifestUrl: '/api/dash/' + asset.id + '/stream.mpd'
					});
					index += 1;
				})
		);
		newImgs = toAdd;
	}

	let lightbox: PhotoSwipeLightbox;
	onMount(() => {
		lightbox = new PhotoSwipeLightbox({
			//showHideAnimationType: 'none',
			pswpModule: () => import('photoswipe'),
			preload: [1, 2]
		});
		const videoPlugin = new PhotoSwipeVideoPlugin(lightbox, {});
		lightbox.addFilter('numItems', (numItems) => {
			return imgs.length;
		});
		lightbox.addFilter('itemData', (itemData, index) => {
			console.log(
				index,
				imgs.map((img) => img.mpdManifestUrl)
			);
			return imgs[index];
		});

		lightbox.addFilter('thumbEl', (thumbEl, data: Image, index) => {
			const el = document.querySelector('[data-img-id="' + data.index + '"] img');
			if (el) {
				return el;
			}
			return thumbEl;
		});
		lightbox.addFilter('placeholderSrc', (placeholderSrc, slide) => {
			let data: Image = slide.data;
			const el = document.querySelector('[data-img-id="' + data.index + '"] img');
			if (el) {
				return el.src;
			}
			return placeholderSrc;
		});
		lightbox.init();
		fetchMore();
	});
</script>

<button
	on:click={() => {
		lightbox.loadAndOpen(2);
	}}>open sv</button
>
<button
	on:click={() => {
		fetchMore();
	}}>load more</button
>

<Gallery
	on:imageClick={(event) => {
		lightbox.loadAndOpen(event.detail.imageIndex);
	}}
	images={imgs}
	rowHeight="120"
	gutter="2"
/>

<style>
	/*
	.scroll {
		box-shadow: 0px 1px 3px 0px rgba(0, 0, 0, 0.2), 0px 1px 1px 0px rgba(0, 0, 0, 0.14),
			0px 2px 1px -1px rgba(0, 0, 0, 0.12);
		display: flex;
		flex-direction: column;
		border-radius: 2px;
		width: 100%;
		max-width: 400px;
		max-height: 400px;
		background-color: white;
		overflow-x: scroll;
		list-style: none;
		padding: 0;
	}*/
</style>
