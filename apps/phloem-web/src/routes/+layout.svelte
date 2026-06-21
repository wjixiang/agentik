<script lang="ts">
	import TopNav from '$lib/components/layout/TopNav.svelte';

	let { children } = $props();
</script>

<TopNav>
	{@render children()}
</TopNav>
