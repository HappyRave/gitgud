<script lang="ts">
	import favicon from '$lib/assets/favicon.svg';
	import TitleBar from '$lib/components/TitleBar.svelte';
	import { settingsStore } from '$lib/stores/settingsStore';
	import '../app.css';

	let { children } = $props();

	function toggleSettings() {
		settingsStore.update(v => !v);
	}
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
</svelte:head>

<div class="flex flex-col h-screen overflow-hidden bg-background text-foreground">
	<TitleBar onSettingsClick={toggleSettings} />
	{@render children()}
</div>
