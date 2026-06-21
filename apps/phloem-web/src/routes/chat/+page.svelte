<script lang="ts">
	import { createChatStore } from '$lib/stores/chat.svelte';

	const chat = createChatStore();
	let inputText = $state('');

	async function handleSend() {
		const text = inputText.trim();
		if (!text || chat.isStreaming) return;
		inputText = '';
		await chat.send(text);
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter' && !e.shiftKey) {
			e.preventDefault();
			handleSend();
		}
	}
</script>

<div class="chat-container">
	<div class="message-list">
		{#each chat.messages as msg (msg.id)}
			<div class="message {msg.role}">
				<div class="message-role">{msg.role === 'user' ? 'You' : 'Agent'}</div>
				<div class="message-content">
					{#if msg.thinking}
						<details class="thinking-block">
							<summary>Thinking</summary>
							<pre>{msg.thinking}</pre>
						</details>
					{/if}
					{#if msg.toolCalls && msg.toolCalls.length > 0}
						<div class="tool-calls">
							{#each msg.toolCalls as tc}
								<details class="tool-call-card">
									<summary>🔧 {tc.name}</summary>
									<pre class="tool-input">{JSON.stringify(tc.input, null, 2)}</pre>
									{#if tc.result}
										<pre class="tool-result">{tc.result.content}</pre>
									{/if}
								</details>
							{/each}
						</div>
					{/if}
					<div class="text-content">
						{msg.content}
						{#if msg.isStreaming}
							<span class="cursor">▌</span>
						{/if}
					</div>
				</div>
			</div>
		{:else}
			<div class="empty-state">
				<p>Send a message to start chatting with the agent.</p>
			</div>
		{/each}
	</div>
	<div class="input-area">
		<textarea
			bind:value={inputText}
			onkeydown={handleKeydown}
			placeholder="Type a message... (Enter to send)"
			rows="3"
		></textarea>
		<button onclick={handleSend} disabled={chat.isStreaming || !inputText.trim()}>
			Send
		</button>
	</div>
</div>

<style>
	.chat-container {
		display: flex;
		flex-direction: column;
		height: 100%;
		max-width: 800px;
		margin: 0 auto;
		padding: 1rem;
	}
	.message-list { flex: 1; overflow-y: auto; padding-bottom: 1rem; }
	.message { margin-bottom: 1rem; }
	.message.user { text-align: right; }
	.message-role { font-size: 0.75rem; color: #94a3b8; margin-bottom: 0.25rem; }
	.message-content {
		display: inline-block;
		text-align: left;
		max-width: 85%;
		padding: 0.75rem 1rem;
		border-radius: 0.5rem;
		background: #f8fafc;
		line-height: 1.6;
		white-space: pre-wrap;
		word-break: break-word;
	}
	.message.user .message-content { background: #eff6ff; }
	.thinking-block { margin-bottom: 0.5rem; font-size: 0.8rem; color: #64748b; }
	.thinking-block pre { white-space: pre-wrap; margin-top: 0.25rem; }
	.tool-calls { margin-bottom: 0.5rem; }
	.tool-call-card {
		background: #fefce8; border: 1px solid #fef08a;
		border-radius: 0.375rem; padding: 0.5rem; margin-bottom: 0.5rem; font-size: 0.8rem;
	}
	.tool-input, .tool-result { white-space: pre-wrap; overflow-x: auto; font-size: 0.75rem; }
	.tool-result { color: #166534; }
	.cursor { animation: blink 0.7s infinite; color: #3b82f6; }
	@keyframes blink { 50% { opacity: 0; } }
	.empty-state { display: flex; align-items: center; justify-content: center; height: 60%; color: #94a3b8; }
	.input-area { display: flex; gap: 0.5rem; padding-top: 1rem; border-top: 1px solid #e2e8f0; }
	.input-area textarea {
		flex: 1; resize: none; padding: 0.75rem;
		border: 1px solid #cbd5e1; border-radius: 0.5rem;
		font-family: inherit; font-size: 0.875rem; line-height: 1.5;
	}
	.input-area textarea:focus { outline: none; border-color: #3b82f6; }
	.input-area button {
		padding: 0 1.5rem; background: #3b82f6; color: white;
		border: none; border-radius: 0.5rem; cursor: pointer; font-size: 0.875rem;
	}
	.input-area button:disabled { opacity: 0.5; cursor: not-allowed; }
</style>
