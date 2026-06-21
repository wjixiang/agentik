<script lang="ts">
	import { page } from '$app/stores';
	import { listTables } from '$lib/api/datalake';

	let namespace = $derived($page.params.namespace);
	let tables = $state<string[]>([]);
	let loading = $state(true);

	async function loadTables() {
		try {
			const result = await listTables(namespace ?? '');
			tables = result.map(t => t.name);
		} catch (e) {
			console.error('Failed to load tables:', e);
		} finally {
			loading = false;
		}
	}

	$effect(() => { loadTables(); });
</script>

<div class="namespace-page">
	<nav class="breadcrumb">
		<a href="/datalake">Data Lake</a> / <span>{namespace}</span>
	</nav>

	{#if loading}
		<p>Loading...</p>
	{:else if tables.length === 0}
		<p>No tables in this namespace.</p>
	{:else}
		<div class="table-list">
			{#each tables as table}
				<a href="/datalake/{namespace}/{table}" class="table-card">
					<span class="icon">📊</span>
					<span class="name">{table}</span>
				</a>
			{/each}
		</div>
	{/if}
</div>

<style>
	.namespace-page { padding: 2rem; max-width: 960px; margin: 0 auto; }
	.breadcrumb { font-size: 0.875rem; color: #64748b; margin-bottom: 1.5rem; }
	.breadcrumb a { color: #3b82f6; text-decoration: none; }
	.table-list { display: flex; flex-direction: column; gap: 0.5rem; }
	.table-card {
		display: flex; align-items: center; gap: 0.75rem;
		padding: 1rem; border: 1px solid #e2e8f0; border-radius: 0.5rem;
		text-decoration: none; color: inherit; transition: border-color 0.15s;
	}
	.table-card:hover { border-color: #3b82f6; }
	.icon { font-size: 1.25rem; }
	.name { font-weight: 500; font-family: monospace; }
</style>
