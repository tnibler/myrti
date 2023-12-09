<script lang="ts" context="module">
	export type Image = ImageSize & {
		src: string;
		thumbSrc: string;
		index: number;
	};
</script>

<script lang="ts">
	import { createEventDispatcher } from 'svelte';
	import layout from './layout';
	import type { ImageSize, ImageLayout } from './layout';

	export let images: Image[] = [];
	export let rowHeight = 220;
	export let gutter = 8;

	let width = 0;
	let laidOutSizes: ImageLayout[];
	let scaledImages: { laidOut: ImageLayout; image: ImageSize }[];

	function imgStyle({ scaledWidth, scaledHeight, isLastInRow, isLastRow }): string {
		let marginRight = gutter + 'px',
			flex = `0 0 ${scaledWidth}px`,
			marginBottom = isLastRow ? '0' : marginRight;

		if (isLastInRow) {
			marginRight = '0';
			flex = `1 1 ${scaledWidth - 4}px`;
		}

		return `height: ${scaledHeight}px; flex: ${flex}; margin-right: ${marginRight}; margin-bottom: ${marginBottom};`;
	}

	$: laidOutSizes = layout({
		images: images,
		containerWidth: width || 1280,
		targetHeight: rowHeight,
		gutter
	});

	$: scaledImages = images.map((image, index) => {
		return {
			laidOut: laidOutSizes[index],
			image
		};
	});
	const dispatch = createEventDispatcher();
</script>

<div class="masonry" bind:clientWidth={width}>
	<div class="container" style="width: {width}px" class:hidden={!width}>
		{#each scaledImages as { laidOut, image }}
			<a
				class="image"
				style={imgStyle({
					scaledWidth: laidOut.scaledWidth,
					scaledHeight: laidOut.scaledHeight,
					isLastRow: laidOut.isLastRow,
					isLastInRow: laidOut.isLastInRow
				})}
				on:click={(e) => {
					dispatch('imageClick', { imageIndex: image.index });
					e.preventDefault();
					return false;
				}}
				data-img-id={image.index}
				target="_blank"
				href={image.src}
			>
				<img src={image.thumbSrc} alt="noaltsorry" style="width:100%; height: 100%" />
			</a>
		{/each}
	</div>
</div>

<style>
	.masonry {
		max-width: 100%;
	}

	.container {
		display: flex;
		flex-wrap: wrap;
	}

	.image {
		position: relative;
		height: 100%;
	}

	.image > :global(*) {
		width: 100%;
		height: 100%;
	}

	.hidden {
		visibility: hidden;
	}
</style>

