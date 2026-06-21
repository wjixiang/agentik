<script lang="ts">
	import { goto } from '$app/navigation';

	$effect(() => {
		goto('/chat', { replaceState: true });
	});
</script>
