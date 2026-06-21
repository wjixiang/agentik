import { apiFetch } from './client';

export interface ServerSettings {
	version: string;
	[key: string]: unknown;
}

export async function getSettings(): Promise<ServerSettings> {
	return apiFetch<ServerSettings>('/api/settings');
}
