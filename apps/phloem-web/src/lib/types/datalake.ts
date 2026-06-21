export interface TableInfo {
	name: string;
	namespace: string;
}

export interface QueryResult {
	columns: string[];
	rows: unknown[][];
	row_count: number;
}
