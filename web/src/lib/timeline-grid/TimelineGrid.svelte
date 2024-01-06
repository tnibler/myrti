<script lang="ts">
	import { api } from '$lib/apiclient';
	import { createTimeline } from '$lib/store/timeline.svelte';
	import GridSection from './GridSection.svelte';
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
	});

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
		/>
	{/each}
</section>

<style>
	#grid {
		position: relative;
	}
</style>
