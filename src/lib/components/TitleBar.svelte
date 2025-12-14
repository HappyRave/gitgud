<script lang="ts">
	import { getCurrentWindow } from '@tauri-apps/api/window';
	import { Minus, Square, X, Moon, Sun, Settings } from 'lucide-svelte';
	import { Button } from '$lib/components/ui/button';
	import { themeStore } from '$lib/stores/themeStore';
	import { onMount } from 'svelte';
	import { browser } from '$app/environment';

	interface TitleBarProps {
		onSettingsClick?: () => void;
	}

	let { onSettingsClick }: TitleBarProps = $props();

	let appWindow: ReturnType<typeof getCurrentWindow> | null = null;
	let isMaximized = $state(false);

	onMount(() => {
		if (browser) {
			appWindow = getCurrentWindow();
			appWindow.isMaximized().then((max) => (isMaximized = max));
		}
	});

	async function minimize() {
		if (appWindow) await appWindow.minimize();
	}

	async function toggleMaximize() {
		if (appWindow) {
			await appWindow.toggleMaximize();
			isMaximized = await appWindow.isMaximized();
		}
	}

	async function close() {
		if (appWindow) await appWindow.close();
	}

	let currentTheme = $derived($themeStore);
	let isDark = $derived(
		currentTheme === 'dark' ||
			(currentTheme === 'system' &&
				browser &&
				typeof window !== 'undefined' &&
				window.matchMedia('(prefers-color-scheme: dark)').matches)
	);
</script>

<div
	data-tauri-drag-region
	class="flex items-center justify-between h-8 bg-background border-b border-border select-none"
	style="-webkit-app-region: drag;"
>
	<div data-tauri-drag-region class="flex items-center gap-2 px-3 flex-1" style="-webkit-app-region: drag;">
		<svg
			class="h-4 w-4 text-primary"
			viewBox="0 0 24 24"
			fill="none"
			xmlns="http://www.w3.org/2000/svg"
		>
			<path
				d="M12 2L2 7L12 12L22 7L12 2Z"
				stroke="currentColor"
				stroke-width="2"
				stroke-linecap="round"
				stroke-linejoin="round"
			/>
			<path
				d="M2 17L12 22L22 17"
				stroke="currentColor"
				stroke-width="2"
				stroke-linecap="round"
				stroke-linejoin="round"
			/>
			<path
				d="M2 12L12 17L22 12"
				stroke="currentColor"
				stroke-width="2"
				stroke-linecap="round"
				stroke-linejoin="round"
			/>
		</svg>
		<span class="text-sm font-semibold text-foreground">gitgud</span>
	</div>

	<div class="flex items-center" style="-webkit-app-region: no-drag;">
		<Button
			variant="ghost"
			size="icon"
			class="h-8 w-8 rounded-none hover:bg-accent"
			onclick={onSettingsClick}
			title="Settings"
		>
			<Settings class="h-4 w-4" />
		</Button>
		<Button
			variant="ghost"
			size="icon"
			class="h-8 w-8 rounded-none hover:bg-accent"
			onclick={() => themeStore.toggle()}
		>
			{#if isDark}
				<Sun class="h-4 w-4" />
			{:else}
				<Moon class="h-4 w-4" />
			{/if}
		</Button>
		<Button
			variant="ghost"
			size="icon"
			class="h-8 w-8 rounded-none hover:bg-accent"
			onclick={minimize}
		>
			<Minus class="h-4 w-4" />
		</Button>
		<Button
			variant="ghost"
			size="icon"
			class="h-8 w-8 rounded-none hover:bg-accent"
			onclick={toggleMaximize}
		>
			<Square class="h-3.5 w-3.5" />
		</Button>
		<Button
			variant="ghost"
			size="icon"
			class="h-8 w-8 rounded-none hover:bg-destructive hover:text-destructive-foreground"
			onclick={close}
		>
			<X class="h-4 w-4" />
		</Button>
	</div>
</div>
