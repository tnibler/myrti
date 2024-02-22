<script lang="ts">
	import Sidebar from './Sidebar.svelte';
</script>

<div id="page">
	<Sidebar />
	<div id="content">
		<slot />
	</div>
</div>

<style>
	#page {
		overflow-y: hidden;
		display: flex;
		height: 100vh;
	}

	#content {
		height: 100vh;
		flex-grow: 1;
	}
	@media screen and (max-width: 600px) {
		#sidebar {
			display: none;
		}
	}
</style>
