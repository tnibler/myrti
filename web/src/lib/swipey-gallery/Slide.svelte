<script context="module">
	export type OpenTransitionParams = {
		fromBounds: ThumbnailBounds;
		onTransitionEnd: () => void;
	};

	export type SlideState = {
		readonly canBePanned: boolean;
		readonly pan: Point;
		readonly panBounds: PanBounds;
		readonly currentZoomLevel: number;
		readonly zoomLevels: ZoomLevels;
		readonly canBeZoomed: boolean;
		readonly size: Size;
	};

	export type SlideControls = SlideState & {
		pan: Point;
		applyCurrentZoomPan: () => void;
		setZoomLevel: (z: number) => void;
		onScrollAway: () => void;
		onActive: () => void;
		closeTransition: (toBounds: ThumbnailBounds, onTransitionEnd: () => void) => void;
	};
</script>

<script lang="ts">
	import { getContext, untrack } from 'svelte';
	import type { Point, Size } from './util_types';
	import { type ZoomLevels, computeZoomLevels } from './zoom';
	import { computePanBounds, type PanBounds } from './pan-bounds';
	import type { ThumbnailBounds } from './thumbnail-bounds';
	import { fade } from 'svelte/transition';
	import type { VideoSlideData, ImageSlideData, SlideData } from './slide-data';
	import SlideImage from './SlideImage.svelte';
	import SlideVideo from './SlideVideo.svelte';
	import './slide.css';
	import type { GalleryControls } from './Pager.svelte';

	type SlideProps = {
		data: SlideData;
		openTransition: OpenTransitionParams | null;
	};
	let { data, openTransition } = $props<SlideProps>();

	let gallery: GalleryControls = getContext('gallery');
	let pan: Point = $state({ x: 0, y: 0 });
	let panAreaSize: Size = $derived(gallery.pager.viewportSize); // TODO missing padding like photoswipe
	let zoomLevels: ZoomLevels = $derived(
		computeZoomLevels({
			maxSize: data.size,
			panAreaSize
		})
	);
	/** Image DOM element has size imgSize * domZoom */
	let domZoom: number = $state(1);
	/** zoom applied as CSS scale on top of domZoom. Only temporary during zoom motions, 
	after that zoom gets applied to the DOM element and transform scale is reset. */
	let cssTransformZoom: number = $state(1);
	const effectiveZoom = $derived(domZoom * cssTransformZoom);
	const panBounds = $derived(computePanBounds(data.size, panAreaSize, effectiveZoom));
	let slideImage: SlideImage | undefined = $state();
	let slideVideo: SlideVideo | undefined = $state();
	let placeholderEl: HTMLImageElement | undefined;
	let isActive = $state(false);

	enum PlaceholderTransition {
		No,
		Running,
		Finished
	}
	let placeholderTransitionState = $state(PlaceholderTransition.No as OpenTransitionState);
	let imageLoaded = $state(false);
	let imageElVisible = $state(false);
	let placeholderVisible = $derived(
		!imageElVisible || placeholderTransitionState === PlaceholderTransition.Running
	);
	/** Wait this long after the real content is ready to hide the placeholder to reveal the <img> underneath.
	Without this, there is a flicker on some devices/browsers. */
	const PLACEHOLDER_HIDE_DELAY = data.type === 'image' ? 450 : 0;

	// for some reason the slideImage/slideVideo bindings don't get unset when the bound component
	// is removed, so we do it manually here
	$effect(() => {
		data.type;
		untrack(() => {
			slideImage = data.type === 'image' ? slideImage : undefined;
			slideVideo = data.type === 'video' ? slideVideo : undefined;
		});
	});

	$effect(() => {
		// small delay between image being loaded and allowed to be shown and actually doing it
		// for flicker reasons. Not perfect but pretty good
		if (
			!imageElVisible &&
			imageLoaded &&
			placeholderTransitionState !== PlaceholderTransition.Running
		) {
			imageElVisible = true;
		}
	});

	let userHasZoomed = $state(false);
	export const controls: SlideControls = {
		get canBePanned() {
			return effectiveZoom > zoomLevels.fit;
		},
		get panBounds() {
			return panBounds;
		},
		get currentZoomLevel() {
			return cssTransformZoom * domZoom;
		},
		set pan(value) {
			pan = value;
		},
		get pan() {
			return pan;
		},
		get zoomLevels() {
			return zoomLevels;
		},
		get canBeZoomed() {
			return true;
		},
		get size() {
			return data.size;
		},
		setZoomLevel: (newZoom) => {
			cssTransformZoom = newZoom / domZoom;
			userHasZoomed = cssTransformZoom > zoomLevels.fit;
		},
		applyCurrentZoomPan: () => {
			domZoom *= cssTransformZoom;
			cssTransformZoom = 1;
		},
		onScrollAway: () => {
			isActive = false;
		},
		onActive: () => {
			isActive = true;
		},
		closeTransition
	};

	let { width, height } = $derived({
		width: data.size.width * domZoom,
		height: data.size.height * domZoom
	});

	$effect(() => {
		const slide = data;
		untrack(() => {
			initializeForNewSlide(slide, panAreaSize);
		});
	});

	$effect(() => {
		const newZoomLevels = computeZoomLevels({
			maxSize: data.size,
			panAreaSize: panAreaSize
		});
		if (effectiveZoom < newZoomLevels.fit || !userHasZoomed) {
			domZoom = newZoomLevels.fit;
			cssTransformZoom = 1;
		}
		const newPanBounds = computePanBounds(data.size, panAreaSize, effectiveZoom);
		pan = {
			x: newPanBounds.center.x,
			y: newPanBounds.center.y
		};
	});

	$effect(() => {
		if (
			openTransition != null &&
			placeholderEl &&
			placeholderTransitionState === PlaceholderTransition.No
		) {
			addOpenTransition(placeholderEl, openTransition);
		}
	});

	function initializeForNewSlide(slide: ImageSlideData, panAreaSize: Size) {
		const newZoomLevels = computeZoomLevels({
			maxSize: untrack(() => slide.size),
			panAreaSize: untrack(() => panAreaSize)
		});
		const newPanBounds = computePanBounds(slide.size, panAreaSize, newZoomLevels.fit);
		domZoom = newZoomLevels.fit;
		cssTransformZoom = 1;
		pan = {
			x: newPanBounds.center.x,
			y: newPanBounds.center.y
		};
	}

	function addOpenTransition(el: HTMLImageElement, t: OpenTransitionParams) {
		const transform = getTransformToFitThumbnail(t.fromBounds);
		el.style.transform = transform;
		placeholderTransitionState = PlaceholderTransition.Running;

		requestAnimationFrame(() => {
			const listener = (e: TransitionEvent) => {
				if (e.target === el) {
					el.removeEventListener('transitionend', listener, false);
					el.removeEventListener('transitioncancel', listener, false);
					placeholderTransitionState = PlaceholderTransition.Finished;
					t.onTransitionEnd();
				}
			};
			el.addEventListener('transitionend', listener, false);
			el.addEventListener('transitioncancel', listener, false);

			// Photoswipe opener.js:249 does werid async waiting for decoding/timeout stuff
			// saying that something doesn't work on firefox.
			// Can't reproduce, this works afaict.
			requestAnimationFrame(() => {
				el.style.transform = '';
			});
		});
	}

	function getTransformToFitThumbnail(bounds: ThumbnailBounds): string {
		if (bounds.crop) {
			console.warn('TODO open anim with cropped bounds not implemented');
		}
		const scaleX = bounds.rect.width / width;
		const scaleY = bounds.rect.height / height;
		const vp = gallery.pager.viewportSize;
		const translateY =
			-vp.height / 2 + bounds.rect.height / 2 + bounds.rect.y + panBounds.center.y - pan.y;

		const translateX =
			-vp.width / 2 + bounds.rect.width / 2 + bounds.rect.x + (panBounds.center.x - pan.x);

		return `translate3d(${translateX}px, ${translateY}px, 0) scale3d(${scaleX}, ${scaleY}, 1)`;
	}

	function closeTransition(toBounds: ThumbnailBounds, onTransitionEnd: () => void) {
		const transform = getTransformToFitThumbnail(toBounds);
		// apply transition to placeholder if the content element hasn't loaded yet
		if (placeholderEl) {
			// The placeholder may still be visible while the content element is already present
			// because of the delay between the image content being ready and the placeholder being hidden.
			// If we close in this intermediary state, the content element should be hidden before animating
			// the placeholder.
			imageElVisible = false;

			const listener = (e: TransitionEvent) => {
				if (e.target === placeholderEl) {
					placeholderEl.removeEventListener('transitionend', listener, false);
					placeholderEl.removeEventListener('transitioncancel', listener, false);
					onTransitionEnd();
				}
			};
			placeholderEl.addEventListener('transitionend', listener, false);
			placeholderEl.addEventListener('transitioncancel', listener, false);
			placeholderTransitionState = PlaceholderTransition.Running;

			requestAnimationFrame(() => {
				if (!placeholderEl) {
					return;
				}
				placeholderEl.style.transform = transform;
			});
		} else if (slideImage) {
			slideImage.closeTransition(transform, onTransitionEnd);
		} else if (slideVideo) {
			slideVideo.closeTransition(transform, onTransitionEnd);
		} else {
			// catch all so gallery closes no matter what weird in-between states we get into
			onTransitionEnd();
		}
	}
</script>

<div
	class="zoom-wrapper"
	style="
  	transform-origin: 0px 0px 0px;
	transform: translate3d({pan.x}px, {pan.y}px, 0) scale3d({cssTransformZoom}, {cssTransformZoom}, 1);"
>
	{#key data.type}
		{#if data.type === 'image'}
			<SlideImage
				bind:this={slideImage}
				size={{ width, height }}
				slideData={data as ImageSlideData}
				isVisible={imageElVisible}
				onContentReady={() => {
					imageLoaded = true;
				}}
			/>
		{:else if data.type === 'video'}
			<SlideVideo
				bind:this={slideVideo}
				size={{ width, height }}
				slideData={data as VideoSlideData}
				isVisible={imageElVisible}
				{isActive}
				onContentReady={() => {
					imageLoaded = true;
				}}
			/>
		{/if}
	{/key}
	{#if placeholderVisible}
		<!-- svelte-ignore a11y-missing-attribute -->
		<img
			class="placeholder"
			bind:this={placeholderEl}
			out:fade={{ duration: 100, delay: PLACEHOLDER_HIDE_DELAY }}
			src={data.placeholderSrc}
			style:width="{width}px"
			style:height="{height}px"
			style:user-select="none"
			class:slide-transition-transform={placeholderTransitionState ===
				PlaceholderTransition.Running}
		/>
	{/if}
</div>

<style>
	.zoom-wrapper {
		transform-origin: 0px 0px 0px;
		position: absolute;
	}

	.placeholder {
		position: absolute;
	}
</style>
