<script lang="ts">
	type Props = {
		title: string;
		iconSvg: string;
		isActive: boolean;
	};

	const { title, iconSvg, isActive } = $props<Props>();
</script>

<div
	class={`w-full flex flex-row px-5 py-2 gap-4 justify-start items-center rounded-r-full ${isActive ? 'bg-blue-100' : 'hover:bg-gray-200'}`}
>
	<svg width="1.5em" height="1.5em" viewBox="0 0 24 24">
		<path d={iconSvg} fill="#000" />
	</svg>
	<p class="font-medium">{title}</p>
</div>
