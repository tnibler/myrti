<script lang="ts">
	import { onMount } from 'svelte';
	import 'photoswipe/photoswipe.css';
	import PhotoSwipeLightbox from 'photoswipe/lightbox';
	import PhotoSwipeVideoPlugin from './photoswipe-shaka.esm';
	import { createAssetStore } from '$lib/store/asset.svelte';
	import dayjs, { Dayjs } from 'dayjs';
	import type { Asset } from '$lib/apitypes';

	const assetStore = createAssetStore();
	const imgs = $derived(buildThumbs(assetStore.assetGroups.flatMap((g) => g.assets)));

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

	function buildThumbs(assets: Asset[]): SlideData<SlideType>[] {
		return assets.map((asset, idx) => {
			if (asset.type === 'image') {
				return <SlideData<'image'>>{
					type: 'image',
					assetId: asset.id,
					index: idx,
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
					index: idx,
					thumbSrc: '/api/asset/thumbnail/' + asset.id + '/large/avif',
					width: asset.width,
					height: asset.height,
					mpdManifestUrl: '/api/dash/' + asset.id + '/stream.mpd'
				};
			}
		});
	}

	async function fetchMore() {
		assetStore.fetchMore();
	}

	function imageClicked(groupIndex: number, imageIndex: number) {
		let imgsBeforeGroup = 0;
		if (groupIndex >= assetStore.assetGroups.length) {
			console.error('groupsIndex >= number of groups!');
		}
		for (let i = 0; i < groupIndex; i++) {
			imgsBeforeGroup += assetStore.assetGroups[i].assets.length;
		}
		lightbox.loadAndOpen(imgsBeforeGroup + imageIndex);
	}

	let lightbox: PhotoSwipeLightbox;
	onMount(() => {
		lightbox = new PhotoSwipeLightbox({
			//showHideAnimationType: 'none',
			pswpModule: () => import('photoswipe')
			// preload: [1, 2]
		});
		const _videoPlugin = new PhotoSwipeVideoPlugin(lightbox, {});
		lightbox.addFilter('numItems', (numItems: unknown) => {
			return imgs.length;
		});
		lightbox.addFilter('itemData', (itemData: SlideData, index: number) => {
			return imgs[index];
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
	});
</script>

<button
	on:click={() => {
		fetchMore();
	}}>load more</button
>

<div>{assetStore.assetGroups.length}</div>

<section>
	{#each assetStore.assetGroups as group, groupIndex}
		<div class="container">
			<span>
				{#if group.type === 'day'}
					{group.date}
				{:else if group.type === 'group'}
					{group.groupTitle}
				{/if}
			</span>
			<ul class="image-gallery">
				{#each group.assets as asset, assetIndex}
					<li>
						<a
							href={'/api/asset/original/' + asset.id}
							data-img-id={asset.id}
							on:click={(e) => {
								e.preventDefault();
								imageClicked(groupIndex, assetIndex);
							}}
						>
							<img
								src={'/api/asset/thumbnail/' + asset.id + '/large/avif'}
								alt=""
								data-img-id={asset.id}
							/>
						</a>
					</li>
				{/each}
			</ul>
		</div>
	{/each}
</section>

<style>
	.image-gallery {
		display: flex;
		flex-wrap: wrap;
		gap: 4px;
	}

	.image-gallery::after {
		content: '';
		flex-grow: 999;
	}

	.image-gallery > li {
		height: 150px;
		cursor: pointer;
		position: relative;
	}

	.image-gallery li img {
		object-fit: cover;
		width: 100%;
		height: 100%;
		vertical-align: middle;
	}

	ul {
		list-style-type: none;
	}
</style>
