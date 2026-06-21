import { apiFetch } from './client';

export async function listNamespaces(): Promise<string[]> {
	return apiFetch<string[]>('/api/datalake/namespaces');
}

export interface TableInfo {
	name: string;
	namespace: string;
}

export async function listTables(namespace: string): Promise<TableInfo[]> {
	return apiFetch<TableInfo[]>(`/api/datalake/namespaces/${namespace}/tables`);
}

export interface QueryResult {
	columns: string[];
	rows: unknown[][];
	row_count: number;
}

export async function queryDatalake(sql: string): Promise<QueryResult> {
	return apiFetch<QueryResult>(`/api/datalake/query?sql=${encodeURIComponent(sql)}`);
}
