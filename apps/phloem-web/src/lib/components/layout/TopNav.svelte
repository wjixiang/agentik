<script lang="ts">
	import type { Snippet } from 'svelte';

	let { children }: { children: Snippet } = $props();

	const navItems = [
		{ href: '/chat', label: 'Chat' },
		{ href: '/agents', label: 'Agents' },
		{ href: '/datalake', label: 'Data Lake' },
		{ href: '/settings', label: 'Settings' }
	];
</script>

<div class="app-shell">
	<nav class="top-nav">
		<div class="nav-brand">
			<span class="brand-icon">🌿</span>
			<span class="brand-name">phloem</span>
		</div>
		<div class="nav-links">
			{#each navItems as item}
				<a href={item.href} class="nav-link">{item.label}</a>
			{/each}
		</div>
	</nav>
	<main class="main-content">
		{@render children()}
	</main>
</div>

<style>
	.app-shell {
		display: flex;
		flex-direction: column;
		height: 100vh;
	}

	.top-nav {
		display: flex;
		align-items: center;
		gap: 2rem;
		padding: 0 1.5rem;
		height: 3rem;
		border-bottom: 1px solid #e2e8f0;
		background: #fff;
	}

	.nav-brand {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		font-weight: 600;
		font-size: 1.1rem;
	}

	.brand-icon {
		font-size: 1.3rem;
	}

	.nav-links {
		display: flex;
		gap: 0.25rem;
	}

	.nav-link {
		padding: 0.5rem 0.75rem;
		border-radius: 0.375rem;
		color: #475569;
		text-decoration: none;
		font-size: 0.875rem;
		transition: background 0.15s, color 0.15s;
	}

	.nav-link:hover {
		background: #f1f5f9;
		color: #0f172a;
	}

	.main-content {
		flex: 1;
		overflow: auto;
	}
</style>
