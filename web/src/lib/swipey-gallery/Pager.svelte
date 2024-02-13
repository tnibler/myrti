<script context="module">
	export type PagerProps = {
		slideIndex: number;
		numSlides: number;
		topOffset: number;
		getSlide: (index: number) => Promise<SlideData>;
		getThumbnailBounds: (index: number) => ThumbnailBounds;
		closeGallery: () => void;
		onOpenTransitionFinished: () => void;
	};

	export type GalleryControls = {
		currentSlide: SlideControls | null;
		pager: PagerControls;
		animations: AnimationControls;
		close: () => void;
		onVerticalDrag: (ratio: number) => void;
	};

	export type PagerState = {
		readonly viewportSize: { width: number; height: number };
		readonly isShifted: boolean;
		readonly currentSlideX: number;
		readonly x: number;
	};

	export type PagerControls = PagerState & {
		moveSlideAnimate: (to: 'left' | 'right' | 'backToCenter') => void;
		moveXBy: (delta: number) => void;
		close: () => void;
	};
</script>

<script lang="ts">
	import SlideHolder from './SlideHolder.svelte';
	import { onMount, setContext, untrack } from 'svelte';
	import { newGestureController } from './gestures';
	import type { SlideData } from './slide-data';
	import { newAnimationControls, type AnimationControls } from './animations';
	import type { ThumbnailBounds } from './thumbnail-bounds';
	import type { OpenTransitionParams, SlideControls } from './Slide.svelte';
	import { fade } from 'svelte/transition';

	let {
		numSlides,
		getSlide,
		slideIndex,
		getThumbnailBounds,
		closeGallery,
		onOpenTransitionFinished,
		topOffset
	} = $props<PagerProps>();

	let viewport = $state({ width: 0, height: 0 });
	const slideSpacing = 0.1;
	const slideWidth = $derived(viewport.width + viewport.width * slideSpacing);

	/** Index of slide we started at, used to compute correct offset of the pager/slide elements */
	let slideIndexInitOffset = $state(0);
	let previousIndex: number = $state(slideIndex);
	let containerShift: number = $state(-1);

	type SlideHolderState = {
		xTransform: number;
		id: number;
		slideIndex: number | null;
		openTransition: OpenTransitionParams | null;
		isActive: boolean;
	};
	// permutation of the SlideHolders that get shuffled while scrolling
	// holderOrder[0] is the index into holderStates for the SlideHolder to the left of the screen,
	// [1] the currently visible one and [2] the one off to the right
	let holderOrder = $state([0, 1, 2]);
	let holderStates: SlideHolderState[] = $state([]);
	let slideControls: SlideControls[] = $state([]);
	let xTransform = $state(0);
	const transformString = $derived(`translate3d(${Math.round(xTransform)}px, 0px, 0px)`);
	let backgroundOpacity = $state(0);
	/** enable CSS transition when assigning backgroundOpacity. Only set on open and close. */
	let backgroundOpacityTransition = $state(true);

	let hasMouse = $state(false);

	const animations: AnimationControls = newAnimationControls();
	const slide: SlideControls = $derived(slideControls[holderOrder[1]]);
	const pagerControls: PagerControls = {
		get viewportSize() {
			return viewport;
		},
		get currentSlideX() {
			return slideWidth * -(slideIndex - slideIndexInitOffset);
		},
		get isShifted() {
			return xTransform !== this.currentSlideX;
		},
		get x() {
			return xTransform;
		},
		moveXBy: (delta) => {
			const SWIPE_END_FRICTION = 0.3;
			if ((slideIndex == 0 && 0 < delta) || (slideIndex == numSlides - 1 && delta < 0)) {
				xTransform += delta * SWIPE_END_FRICTION;
			} else {
				xTransform += delta;
			}
		},
		moveSlideAnimate,
		close
	};
	const gallery: GalleryControls = {
		get currentSlide() {
			return slide;
		},
		get pager() {
			return pagerControls;
		},
		get animations() {
			return animations;
		},
		close: () => {
			closeGallery();
		},
		onVerticalDrag: (ratio) => {
			backgroundOpacity = 1 - ratio;
		}
	};
	setContext('gallery', gallery);

	let pagerWrapper: HTMLElement;

	onMount(() => {
		const idxs = [
			slideIndex === 0 ? null : slideIndex - 1,
			slideIndex,
			slideIndex === numSlides - 1 ? null : slideIndex + 1
		];
		slideIndexInitOffset = slideIndex;
		const openTransition = {
			onTransitionEnd: afterOpenTransition,
			fromBounds: getThumbnailBounds(slideIndex)
		};
		// holderOrder is the identity mapping at the beginning, so id == index initially for the SlideHolders
		holderStates = [0, 1, 2].map((id) => {
			return {
				// maybe hide left and right holders until open anim finished? see main-scroll.js:111
				id: id,
				xTransform: (id - 1) * slideWidth,
				slideIndex: idxs[id],
				openTransition: id === 1 ? openTransition : null,
				isActive: id === 1
			};
		});
		backgroundOpacity = 1;
		bindEvents();
		return () => {
			unbindEvents();
		};
	});

	// update SlideHolder x position when viewport width changes
	$effect(() => {
		viewport.width;
		untrack(() => {
			for (let i = 0; i < 3; i += 1) {
				const holderState = holderStates[holderOrder[i]];
				holderState.xTransform = (containerShift + i) * slideWidth;
			}
		});
	});

	function afterOpenTransition() {
		backgroundOpacityTransition = false;
		onOpenTransitionFinished();
	}

	function bindEvents() {
		const onMouseDetected = () => {
			hasMouse = true;
		};
		let gestureController = newGestureController(gallery, onMouseDetected);
		pagerWrapper.onpointerdown = gestureController.onPointerDown;
		window.onpointerup = gestureController.onPointerUp;
		window.onpointermove = gestureController.onPointerMove;
		pagerWrapper.onpointercancel = gestureController.onPointerUp;
		pagerWrapper.onclick = gestureController.onClick;
	}

	function unbindEvents() {
		pagerWrapper.onpointerdown = null;
		window.onpointerup = null;
		window.onpointermove = null;
		pagerWrapper.onpointercancel = null;
		pagerWrapper.onclick = null;
	}

	function moveSlideAnimate(direction: 'left' | 'right' | 'backToCenter') {
		let diff = 0;
		if (direction === 'left') {
			diff = -1;
		} else if (direction === 'right') {
			diff = 1;
		}
		const index = Math.min(Math.max(slideIndex + diff, 0), numSlides - 1);
		if (index !== slideIndex) {
			slide.onScrollAway();
		}
		const destX = -(index - slideIndexInitOffset) * slideWidth;
		animations.stopAnimationsFor('pager');
		animations.startSpringAnimation(
			{
				start: xTransform,
				end: destX,
				velocity: 0,
				frequency: 30,
				dampingRatio: 1, //0.7,
				onUpdate: (x: number) => {
					xTransform = x;
				},
				onFinish: () => {
					if (direction !== 'backToCenter') {
						previousIndex = slideIndex;
						slideIndex = index;
						reorderItemHoldersAfterAnim();
					}
				}
			},
			'pager'
		);
	}

	function moveSlide(direction: 'left' | 'right') {
		animations.stopAllAnimations();
		let diff = 0;
		if (direction === 'left') {
			diff = -1;
		} else if (direction === 'right') {
			diff = 1;
		}
		const newIndex = Math.min(Math.max(slideIndex + diff, 0), numSlides - 1);
		if (newIndex !== slideIndex) {
			slide.onScrollAway();
		}
		previousIndex = slideIndex;
		slideIndex = newIndex;
		const destX = -(newIndex - slideIndexInitOffset) * slideWidth;
		xTransform = destX;
		reorderItemHoldersAfterAnim();
	}

	function reorderItemHoldersAfterAnim() {
		animations.stopAnimationsFor('pan');
		const diffMod3 = (slideIndex - previousIndex) % 3;
		const previousActiveHolder: SlideHolderState = holderStates[holderOrder[1]];
		let movedHolder: SlideHolderState;
		// TODO Photoswipe resets transforms here if containerShiftIndex >= 50
		if (diffMod3 === 1 || diffMod3 === -2) {
			containerShift += 1;
			holderOrder = [holderOrder[1], holderOrder[2], holderOrder[0]];
			movedHolder = holderStates[holderOrder[2]];
			movedHolder.xTransform = (containerShift + 2) * slideWidth;
		} else if (diffMod3 === 2 || diffMod3 === -1) {
			containerShift -= 1;
			holderOrder = [holderOrder[2], holderOrder[0], holderOrder[1]];
			movedHolder = holderStates[holderOrder[0]];
			movedHolder.xTransform = containerShift * slideWidth;
		} else if (diffMod3 === 0) {
			// nothing to do
			return;
		} else {
			console.assert(false, 'unreachable!');
			return;
		}
		const newActiveHolder = holderStates[holderOrder[1]];
		previousActiveHolder.isActive = false;
		newActiveHolder.isActive = true;
		slide.onActive();
		const currentSlideIndex = newActiveHolder.slideIndex;
		console.assert(
			currentSlideIndex !== null,
			'currentSlideIndex is null after shuffling SlideHolders'
		);
		if (currentSlideIndex !== null) {
			const nextSlideIndex = currentSlideIndex == numSlides - 1 ? null : currentSlideIndex + 1;
			movedHolder.slideIndex = nextSlideIndex;
		}
		movedHolder.openTransition = null;
	}

	export async function close() {
		const thumbnailBounds = getThumbnailBounds(slideIndex);
		const slide = slideControls[holderOrder[1]];
		backgroundOpacityTransition = true;
		// requestAnimationFrame(() => {
		backgroundOpacity = 0;
		// });
		const p = new Promise<void>((resolve) => {
			slide.closeTransition(thumbnailBounds, () => {
				resolve();
			});
		});
		return p;
	}
</script>

<!--Taken from photoswipe util/viewport-size.js getViewportSize -->
<svelte:window bind:innerHeight={viewport.height} bind:innerWidth={viewport.width} />
<!-- VV errors out, idk svelte 5 bug? -->
<!-- <svelte:document bind:clientWidth={viewport.width} /> -->

<div class="pager-wrapper" bind:this={pagerWrapper} style:top={`${topOffset}px`}>
	<div
		class="background"
		style:opacity={backgroundOpacity}
		class:transition-opacity={backgroundOpacityTransition}
	/>
	<div class="container" style="transform: {transformString};">
		{#each holderStates as slideHolder (slideHolder.id)}
			<SlideHolder
				id={slideHolder.id}
				isActive={slideHolder.isActive}
				xTransform={slideHolder.xTransform}
				openTransition={slideHolder.openTransition}
				slide={slideHolder.slideIndex !== null ? getSlide(slideHolder.slideIndex) : null}
				bind:slideControls={slideControls[slideHolder.id]}
			/>
		{/each}
	</div>
	<button
		class="icon-button"
		class:button-visible={hasMouse}
		in:fade
		onclick={() => moveSlide('left')}
	>
		<svg class="arrow-icon" id="arrow" viewBox="0 0 60 60" width="60" height="60"
			><path d="M29 43l-3 3-16-16 16-16 3 3-13 13 13 13z"></path></svg
		>
	</button>
	<button
		class="icon-button arrow-right"
		class:button-visible={hasMouse}
		in:fade
		onclick={() => moveSlide('right')}
	>
		<svg class="arrow-icon arrow-right" viewBox="0 0 60 60" width="60" height="60"
			><use class="" xlink:href="#arrow"></use></svg
		>
	</button>
	<button
		class="close-button"
		class:button-visible={hasMouse}
		in:fade
		onclick={() => closeGallery()}
	>
		<svg class="arrow-icon" id="arrow" viewBox="0 0 60 60" width="60" height="60">
			<path d="M24 10l-2-2-6 6-6-6-2 2 6 6-6 6 2 2 6-6 6 6 2-2-6-6z"></path>
		</svg>
	</button>
</div>

<style>
	.pager-wrapper,
	.background,
	.container {
		position: absolute;
		left: 0;
		width: 100%;
		height: 100%;
	}

	.background,
	.container {
		position: absolute;
		top: 0;
		left: 0;
		width: 100%;
		height: 100%;
	}

	.background {
		will-change: opacity;
		background: #000;
	}

	.background.transition-opacity {
		transition: opacity 200ms ease-in-out;
	}

	.pager-wrapper {
		overflow: hidden;
		touch-action: none;
	}

	.container {
		user-select: none;
	}

	.close-button {
		display: none;
		position: absolute;
		overflow: hidden;
		background: none;
		box-shadow: none;
		border: 0;

		top: 10px;
		right: 0px;

		pointer-events: auto;
		cursor: pointer;

		-webkit-appearance: none;
		appearance: none;
	}

	.icon-button {
		display: none;
		position: absolute;
		overflow: hidden;
		background: none;
		box-shadow: none;
		border: 0;
		height: 100%;
		padding-left: 10px;
		padding-right: 100px;

		pointer-events: auto;
		cursor: pointer;

		-webkit-appearance: none;
		appearance: none;
	}

	.arrow-icon {
		fill: white;
		opacity: 0.8;
	}

	.icon-button.arrow-right {
		right: 0px;
		padding-right: 10px;
		padding-left: 100px;
	}

	.arrow-icon.arrow-right {
		transform: scale(-1, 1);
		right: 0px;
	}

	.button-visible {
		display: block;
	}
</style>
