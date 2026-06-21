import { subscribeSSE } from '$lib/utils/sse-parser';
import type { SseEventType } from '$lib/utils/sse-parser';

/** Reactive SSE connection state. */
let currentCleanup: (() => void) | null = null;

export function connectAgentStream(
	agentId: string,
	handler: (type: SseEventType, data: string) => void
) {
	disconnect();
	currentCleanup = subscribeSSE(`/api/chat/${agentId}/stream`, (event) => {
		handler(event.type, event.data);
	});
}

export function disconnect() {
	if (currentCleanup) {
		currentCleanup();
		currentCleanup = null;
	}
}
