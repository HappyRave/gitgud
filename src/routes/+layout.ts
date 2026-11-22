import "../app.css";

// No Node server in Tauri: adapter-static serves the app as an SPA.
export const ssr = false;

export async function load() {
  // `pnpm dev` + `/?mock` or `/?mock=merge`: a fake backend to look at the UI in a plain browser.
  if (import.meta.env.DEV) {
    const mock = new URLSearchParams(window.location.search).get("mock");
    if (mock !== null) {
      const { installMock } = await import("$lib/dev/mock");
      installMock(mock);
    }
  }
}
