/**
 * Stops WebView2's own browser menu (Back, Refresh, Print, Inspect…) everywhere but text
 * fields, where its Cut / Copy / Paste is useful. gitgud's own menus run before this.
 */
export function blockBrowserMenu(event: MouseEvent) {
  const target = event.target;
  if (target instanceof HTMLElement && target.closest("input, textarea, [contenteditable]:not([contenteditable='false'])")) {
    return;
  }
  event.preventDefault();
}
