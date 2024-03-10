<script lang="ts">
	import type { MouseEventHandler } from 'svelte/elements';

	type Props = {
		text: string;
		onclick: MouseEventHandler<HTMLButtonElement> | undefined;
		primary: boolean | undefined;
		class: string | undefined;
	};
	const { text, onclick, primary = undefined, ...otherProps } = $props<Props>();
</script>

<button
	class="w-fit py-2 px-3 rounded-md transition-all font-medium text-md {primary
		? 'bg-purple-300 hover:shadow-md'
		: 'bg-transparent hover:bg-gray-200'} {otherProps['class']}"
	{onclick}
>
	{text}
</button>
