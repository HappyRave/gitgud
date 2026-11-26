import { type Editor, openInEditor, openTerminal, revealPath } from "$lib/api";
import type { MenuItem } from "$lib/components/ContextMenu.svelte";
import { type EditorChoice, prefs, type TerminalChoice } from "$lib/prefs.svelte";

export const TERMINAL_NAME: Record<TerminalChoice, string> = {
  windowsTerminal: "Windows Terminal",
  powerShell: "PowerShell",
  gitBash: "Git Bash",
  cmd: "Command Prompt",
};

export const EDITOR_NAME: Record<EditorChoice, string> = {
  vsCode: "VS Code",
  rider: "Rider",
  visualStudio: "Visual Studio",
  custom: "editor",
};

/** The editor chosen in Settings › Tools, as the backend takes it. */
export function chosenEditor(): Editor {
  return prefs.editor === "custom" ? { kind: "custom", program: prefs.editorProgram } : { kind: prefs.editor };
}

/** The custom editor's file name ("notepad++"), else the editor's name. */
export function editorName(): string {
  if (prefs.editor !== "custom") {
    return EDITOR_NAME[prefs.editor];
  }
  const program = prefs.editorProgram.trim().replace(/^"|"$/g, "");
  const base = program.slice(Math.max(program.lastIndexOf("\\"), program.lastIndexOf("/")) + 1).replace(/\.exe$/i, "");
  return base || "editor";
}

/**
 * Show in Explorer / Open terminal / Open in editor / Copy path for `path` in the repo at `root`
 * (null: the repo itself). `onerror` gets the backend's refusal (editor not installed…).
 */
export function launchItems(
  root: string,
  path: string | null,
  onerror: (message: string) => void,
  { onDisk = true }: { onDisk?: boolean } = {},
): MenuItem[] {
  const fail = (err: unknown) => onerror(String(err));
  const items: MenuItem[] = [
    { label: "Show in Explorer", run: () => revealPath(root, path).catch(fail) },
    {
      label: path ? `Open ${TERMINAL_NAME[prefs.terminal]} in its folder` : `Open ${TERMINAL_NAME[prefs.terminal]} here`,
      run: () => openTerminal(root, path, prefs.terminal).catch(fail),
    },
    {
      label: `Open in ${editorName()}`,
      disabled: !onDisk,
      run: () => openInEditor(root, path, chosenEditor()).catch(fail),
    },
  ];
  if (path) {
    const full = `${root.replace(/[\\/]+$/, "")}\\${path.replaceAll("/", "\\")}`;
    items.push({ label: "Copy path", run: () => navigator.clipboard.writeText(full).catch(fail) });
  }
  return items;
}
