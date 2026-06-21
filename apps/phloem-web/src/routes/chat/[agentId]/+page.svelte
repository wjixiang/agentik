<script lang="ts">
	import { createChatStore } from '$lib/stores/chat.svelte';
	import { page } from '$app/stores';

	const chat = createChatStore();
	let agentId = $derived($page.params.agentId);

	$effect(() => {
		chat.setActiveAgent(agentId ?? null);
	});
</script>

<div class="agent-chat">
	<div class="agent-header">
		<span>Agent: {agentId}</span>
		<a href="/agents">← All Agents</a>
	</div>
	<div class="placeholder">
		<p>Chat with agent {agentId}</p>
		<p>Full chat UI coming soon...</p>
	</div>
</div>

<style>
	.agent-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 1rem;
		border-bottom: 1px solid #e2e8f0;
	}
	.placeholder {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		height: 60%;
		color: #94a3b8;
	}
</style>
