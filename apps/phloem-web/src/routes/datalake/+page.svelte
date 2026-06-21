<script lang="ts">
	import { listNamespaces } from '$lib/api/datalake';

	let namespaces = $state<string[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);

	async function loadNamespaces() {
		try {
			namespaces = await listNamespaces();
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load namespaces';
		} finally {
			loading = false;
		}
	}

	$effect(() => { loadNamespaces(); });
</script>

<div class="datalake-page">
	<h1>Data Lake</h1>

	{#if loading}
		<p>Loading...</p>
	{:else if error}
		<p class="error">{error}</p>
	{:else if namespaces.length === 0}
		<div class="empty-state">
			<p>No namespaces found. Configure your Iceberg catalog in <a href="/settings">Settings</a>.</p>
		</div>
	{:else}
		<div class="namespace-list">
			{#each namespaces as ns}
				<a href="/datalake/{ns}" class="namespace-card">
					<span class="icon">📁</span>
					<span class="name">{ns}</span>
				</a>
			{/each}
		</div>
	{/if}
</div>

<style>
	.datalake-page { padding: 2rem; max-width: 960px; margin: 0 auto; }
	h1 { margin-bottom: 1.5rem; }
	.error { color: #dc2626; }
	.empty-state { color: #94a3b8; margin-top: 2rem; }
	.namespace-list { display: flex; flex-direction: column; gap: 0.5rem; }
	.namespace-card {
		display: flex; align-items: center; gap: 0.75rem;
		padding: 1rem; border: 1px solid #e2e8f0; border-radius: 0.5rem;
		text-decoration: none; color: inherit; transition: border-color 0.15s;
	}
	.namespace-card:hover { border-color: #3b82f6; }
	.icon { font-size: 1.25rem; }
	.name { font-weight: 500; font-family: monospace; }
</style>
