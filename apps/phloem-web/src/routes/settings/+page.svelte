<script lang="ts">
	import { getSettings } from '$lib/api/settings';

	let settings = $state<Record<string, unknown> | null>(null);
	let loading = $state(true);

	async function loadSettings() {
		try {
			settings = await getSettings();
		} catch (e) {
			console.error('Failed to load settings:', e);
		} finally {
			loading = false;
		}
	}

	$effect(() => { loadSettings(); });
</script>

<div class="settings-page">
	<h1>Settings</h1>

	{#if loading}
		<p>Loading...</p>
	{:else if settings}
		<div class="settings-card">
			<h2>Server</h2>
			<p>Version: {String(settings.version ?? 'unknown')}</p>
		</div>

		<div class="settings-card">
			<h2>Providers</h2>
			<p class="placeholder">Provider configuration coming soon...</p>
		</div>

		<div class="settings-card">
			<h2>Model Pool</h2>
			<p class="placeholder">Model pool management coming soon...</p>
		</div>
	{/if}
</div>

<style>
	.settings-page { padding: 2rem; max-width: 720px; margin: 0 auto; }
	h1 { margin-bottom: 1.5rem; }
	.settings-card {
		border: 1px solid #e2e8f0; border-radius: 0.5rem;
		padding: 1.25rem; margin-bottom: 1rem;
	}
	.settings-card h2 { font-size: 1rem; margin-bottom: 0.75rem; }
	.placeholder { color: #94a3b8; font-style: italic; }
</style>
