/** Messages in the chat view, mirrored from agentik-types AgentEvent */
export interface ChatMessage {
	id: string;
	role: 'user' | 'assistant';
	content: string;
	thinking?: string;
	toolCalls?: ToolCallInfo[];
	isStreaming?: boolean;
	timestamp: number;
}

export interface ToolCallInfo {
	name: string;
	input: unknown;
	result?: ToolResultInfo;
}

export interface ToolResultInfo {
	ok: boolean;
	content: string;
}
