import { browser } from '$app/environment';
import { writable } from 'svelte/store';

type Theme = 'light' | 'dark' | 'system';

const THEME_KEY = 'gitgud-theme';

function getInitialTheme(): Theme {
	if (!browser) return 'system';
	return (localStorage.getItem(THEME_KEY) as Theme) || 'system';
}

function getSystemTheme(): 'light' | 'dark' {
	if (!browser) return 'light';
	return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
}

function createThemeStore() {
	const { subscribe, set } = writable<Theme>(getInitialTheme());

	function applyTheme(theme: Theme) {
		if (!browser) return;

		const actualTheme = theme === 'system' ? getSystemTheme() : theme;
		const root = document.documentElement;

		if (actualTheme === 'dark') {
			root.classList.add('dark');
		} else {
			root.classList.remove('dark');
		}
	}

	// Initialize theme immediately
	if (browser) {
		const initialTheme = getInitialTheme();
		applyTheme(initialTheme);

		// Listen for system theme changes
		window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', () => {
			const currentTheme = localStorage.getItem(THEME_KEY) as Theme;
			if (currentTheme === 'system' || !currentTheme) {
				applyTheme('system');
			}
		});
	}

	return {
		subscribe,
		setTheme: (theme: Theme) => {
			if (browser) {
				localStorage.setItem(THEME_KEY, theme);
				applyTheme(theme);
			}
			set(theme);
		},
		toggle: () => {
			const currentTheme = getInitialTheme();
			const actualTheme = currentTheme === 'system' ? getSystemTheme() : currentTheme;
			const newTheme = actualTheme === 'dark' ? 'light' : 'dark';
			if (browser) {
				localStorage.setItem(THEME_KEY, newTheme);
				applyTheme(newTheme);
			}
			set(newTheme);
		}
	};
}

export const themeStore = createThemeStore();
