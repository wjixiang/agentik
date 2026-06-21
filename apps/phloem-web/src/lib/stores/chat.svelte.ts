import type { ChatMessage, ToolCallInfo } from '$lib/types/chat';
import { connectAgentStream, disconnect } from './sse.svelte';
import { sendMessage } from '$lib/api/chat';

let nextId = 0;
function makeId(): string {
	return `msg-${nextId++}`;
}

/** Chat state — each page creates its own instance. */
export function createChatStore() {
	let messages = $state<ChatMessage[]>([]);
	let activeAgentId = $state<string | null>(null);
	let isStreaming = $state(false);

	function addUserMessage(content: string) {
		messages.push({
			id: makeId(),
			role: 'user',
			content,
			isStreaming: false,
			timestamp: Date.now()
		});
	}

	function createAssistantMessage(): ChatMessage {
		const msg: ChatMessage = {
			id: makeId(),
			role: 'assistant',
			content: '',
			thinking: '',
			toolCalls: [],
			isStreaming: true,
			timestamp: Date.now()
		};
		messages.push(msg);
		isStreaming = true;
		return msg;
	}

	function handleSseEvent(msg: ChatMessage, type: string, data: string) {
		switch (type) {
			case 'text_delta':
				msg.content += data;
				break;
			case 'thinking_delta':
				msg.thinking = (msg.thinking ?? '') + data;
				break;
			case 'tool_call': {
				const parsed = JSON.parse(data) as ToolCallInfo;
				msg.toolCalls = [...(msg.toolCalls ?? []), parsed];
				break;
			}
			case 'tool_result': {
				const result = JSON.parse(data) as { ok: boolean; content: string };
				if (msg.toolCalls && msg.toolCalls.length > 0) {
					const last = msg.toolCalls[msg.toolCalls.length - 1];
					last.result = result;
				}
				break;
			}
			case 'done':
			case 'error':
				msg.isStreaming = false;
				isStreaming = false;
				if (type === 'error') {
					msg.content += `\n\nError: ${data}`;
				}
				break;
		}
	}

	async function send(content: string, identity?: string) {
		addUserMessage(content);
		const assistantMsg = createAssistantMessage();

		const res = await sendMessage({
			agent_id: activeAgentId ?? undefined,
			identity,
			content
		});

		if (!activeAgentId) {
			activeAgentId = res.agent_id;
		}

		connectAgentStream(res.agent_id, (eventType, eventData) => {
			handleSseEvent(assistantMsg, eventType, eventData);
		});
	}

	function closeStream() {
		disconnect();
		isStreaming = false;
	}

	function setActiveAgent(id: string | null) {
		activeAgentId = id;
	}

	return {
		get messages() { return messages; },
		get activeAgentId() { return activeAgentId; },
		get isStreaming() { return isStreaming; },
		send,
		setActiveAgent,
		closeStream
	};
}
