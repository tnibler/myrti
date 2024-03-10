<script lang="ts">
	import type { Snippet } from 'svelte';
	import AppBar from './appbar/AppBar.svelte';
	import Sidebar, { type SidebarEntry } from '../routes/Sidebar.svelte';

	type Props = {
		content: Snippet;
		appBarOverride: Snippet | undefined;
		showAppBarOverride: boolean;
		activeSideBarEntry: SidebarEntry;
	};

	const {
		content,
		appBarOverride = undefined,
		showAppBarOverride,
		activeSideBarEntry
	} = $props<Props>();
</script>

<div class="flex flex-col h-dvh">
	{#if showAppBarOverride && appBarOverride}
		{@render appBarOverride()}
	{:else}
		<AppBar />
	{/if}
	<div id="page" class="flex-1">
		<Sidebar activeEntry={activeSideBarEntry} />
		<div id="content">
			{@render content()}
		</div>
	</div>
</div>

<style>
	#page {
		overflow-y: hidden;
		display: flex;
	}

	#content {
		flex-grow: 1;
	}

	@media screen and (max-width: 600px) {
		#sidebar {
			display: none;
		}
	}
</style>
