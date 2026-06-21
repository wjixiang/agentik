import { apiFetch } from './client';

export interface ChatRequest {
	agent_id?: string;
	identity?: string;
	content: string;
}

export interface ChatResponse {
	agent_id: string;
	status: string;
}

export interface AgentInfo {
	id: string;
	identity: string;
	status: string;
}

export async function sendMessage(req: ChatRequest): Promise<ChatResponse> {
	return apiFetch<ChatResponse>('/api/chat', {
		method: 'POST',
		body: JSON.stringify(req)
	});
}

export async function listAgents(): Promise<AgentInfo[]> {
	return apiFetch<AgentInfo[]>('/api/agents');
}
