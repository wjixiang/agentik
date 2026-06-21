<script lang="ts">
	import { listAgents } from '$lib/api/chat';
	import type { AgentInfo } from '$lib/types/agent';

	let agents = $state<AgentInfo[]>([]);
	let loading = $state(true);

	async function loadAgents() {
		try {
			agents = await listAgents();
		} catch (e) {
			console.error('Failed to load agents:', e);
		} finally {
			loading = false;
		}
	}

	$effect(() => { loadAgents(); });
</script>

<div class="agents-page">
	<h1>Agent Pool</h1>

	{#if loading}
		<p>Loading...</p>
	{:else if agents.length === 0}
		<div class="empty-state">
			<p>No agents running. Send a message from the <a href="/chat">Chat</a> page to create one.</p>
		</div>
	{:else}
		<div class="agent-grid">
			{#each agents as agent}
				<a href="/chat/{agent.id}" class="agent-card">
					<div class="agent-id">{agent.id.slice(0, 8)}...</div>
					<div class="agent-identity">{agent.identity}</div>
					<div class="agent-status">
						<span class="status-dot"></span>
						{agent.status}
					</div>
				</a>
			{/each}
		</div>
	{/if}
</div>

<style>
	.agents-page { padding: 2rem; max-width: 960px; margin: 0 auto; }
	h1 { margin-bottom: 1.5rem; }
	.empty-state { color: #94a3b8; margin-top: 2rem; }
	.agent-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(280px, 1fr)); gap: 1rem; }
	.agent-card {
		display: block; padding: 1.25rem; border: 1px solid #e2e8f0;
		border-radius: 0.5rem; text-decoration: none; color: inherit;
		transition: border-color 0.15s, box-shadow 0.15s;
	}
	.agent-card:hover { border-color: #3b82f6; box-shadow: 0 2px 8px rgba(59,130,246,0.1); }
	.agent-id { font-family: monospace; font-size: 0.8rem; color: #64748b; }
	.agent-identity { margin-top: 0.5rem; font-weight: 500; }
	.agent-status { margin-top: 0.75rem; font-size: 0.8rem; display: flex; align-items: center; gap: 0.5rem; }
	.status-dot { width: 8px; height: 8px; border-radius: 50%; background: #22c55e; }
</style>
