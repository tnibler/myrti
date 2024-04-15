<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { MouseEventHandler } from 'svelte/elements';

	type Props = {
		text: string;
		onclick: MouseEventHandler<HTMLButtonElement>;
		class?: string;
		children: Snippet;
	};
	const { text, onclick, children, ...otherProps }: Props = $props();
</script>

<button
	class="flex flex-row items-center gap-2
          w-fit py-2 px-3 rounded-md font-medium text-md
          transition-all duration-75
          border border-black/60
          hover:bg-black/20 active:bg-black/30
          {otherProps['class']}"
	{onclick}
>
	{@render children()}
	{text}
</button>
