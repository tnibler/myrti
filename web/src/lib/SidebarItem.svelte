<script lang="ts">
	import type { Snippet } from 'svelte';

	type Props = {
		title: string;
		isActive: boolean;
		children: Snippet;
	};

	const { title, isActive, children }: Props = $props();
</script>

<div
	class={`w-full flex flex-row px-5 py-2 gap-5 justify-start items-center rounded-r-full ${isActive ? 'bg-blue-100' : 'hover:bg-gray-200'}`}
>
	{@render children()}
	<p class="font-medium text-lg">{title}</p>
</div>
