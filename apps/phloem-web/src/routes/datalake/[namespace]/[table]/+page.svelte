<script lang="ts">
	import { page } from '$app/stores';
	import { queryDatalake } from '$lib/api/datalake';

	let sql = $state(`SELECT * FROM "${$page.params.namespace}"."${$page.params.table}" LIMIT 50`);
	let namespace = $derived($page.params.namespace);
	let table = $derived($page.params.table);
	let result = $state<{ columns: string[]; rows: unknown[][]; row_count: number } | null>(null);
	let running = $state(false);

	async function runQuery() {
		running = true;
		try {
			result = await queryDatalake(sql);
		} catch (e) {
			console.error('Query failed:', e);
		} finally {
			running = false;
		}
	}
</script>

<div class="table-page">
	<nav class="breadcrumb">
		<a href="/datalake">Data Lake</a> /
		<a href="/datalake/{namespace}">{namespace}</a> /
		<span>{table}</span>
	</nav>

	<div class="query-editor">
		<textarea bind:value={sql} rows="3" placeholder="Enter SQL query..."></textarea>
		<button onclick={runQuery} disabled={running}>
			{running ? 'Running...' : 'Run Query'}
		</button>
	</div>

	{#if result}
		<div class="result-info">
			{result.row_count} rows, {result.columns.length} columns
		</div>
		<div class="result-table-wrapper">
			<table>
				<thead>
					<tr>
						{#each result.columns as col}
							<th>{col}</th>
						{/each}
					</tr>
				</thead>
				<tbody>
					{#each result.rows as row}
						<tr>
							{#each row as cell}
								<td>{String(cell ?? 'null')}</td>
							{/each}
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
	{/if}
</div>

<style>
	.table-page { padding: 2rem; max-width: 1200px; margin: 0 auto; }
	.breadcrumb { font-size: 0.875rem; color: #64748b; margin-bottom: 1.5rem; }
	.breadcrumb a { color: #3b82f6; text-decoration: none; }
	.query-editor { display: flex; gap: 0.5rem; margin-bottom: 1.5rem; }
	.query-editor textarea {
		flex: 1; padding: 0.75rem; border: 1px solid #cbd5e1;
		border-radius: 0.5rem; font-family: monospace; font-size: 0.875rem;
	}
	.query-editor textarea:focus { outline: none; border-color: #3b82f6; }
	.query-editor button {
		padding: 0 1.5rem; background: #3b82f6; color: white;
		border: none; border-radius: 0.5rem; cursor: pointer;
	}
	.query-editor button:disabled { opacity: 0.5; cursor: not-allowed; }
	.result-info { font-size: 0.8rem; color: #64748b; margin-bottom: 0.5rem; }
	.result-table-wrapper { overflow-x: auto; border: 1px solid #e2e8f0; border-radius: 0.5rem; }
	table { width: 100%; border-collapse: collapse; font-size: 0.8rem; }
	th { background: #f8fafc; padding: 0.5rem; text-align: left; border-bottom: 1px solid #e2e8f0; }
	td { padding: 0.5rem; border-bottom: 1px solid #f1f5f9; }
</style>
