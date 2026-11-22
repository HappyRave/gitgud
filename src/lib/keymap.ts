export interface ShortcutItem {
  keys: string;
  label: string;
}

export interface ShortcutGroup {
  category: "Navigation" | "Working copy" | "Branches & remotes" | "History & diff" | "Application";
  items: ShortcutItem[];
}

/** Single source of truth for shortcuts displayed in `?` sheet and command palette hints. */
export const SHORTCUT_GROUPS: ShortcutGroup[] = [
  {
    category: "Navigation",
    items: [
      { keys: "Ctrl+K", label: "Command palette" },
      { keys: "Ctrl+P", label: "Quick-switch repository" },
      { keys: "Ctrl+1 / 2 / 3", label: "Focus Working Copy / Graph / History" },
      { keys: "F6 / Shift+F6", label: "Cycle panes" },
      { keys: "Ctrl+Tab", label: "Switch open tabs" },
      { keys: "Ctrl+W", label: "Close repository tab" },
      { keys: "Ctrl+F", label: "Find in graph" },
    ],
  },
  {
    category: "Working copy",
    items: [
      { keys: "↑ / ↓", label: "Previous / next file" },
      { keys: "Space / Enter", label: "Stage or unstage file" },
      { keys: "A", label: "Stage all changes" },
      { keys: "Shift+⌫", label: "Discard file changes (undoable)" },
      { keys: "C", label: "Focus commit message" },
      { keys: "Alt+↑ / Alt+↓", label: "Previous / next hunk" },
      { keys: "S / U / ⌫", label: "Stage / unstage / discard hunk" },
      { keys: "Click / Shift+Click", label: "Select lines in diff to stage or discard" },
      { keys: "Ctrl+Enter", label: "Commit staged changes" },
    ],
  },
  {
    category: "Branches & remotes",
    items: [
      { keys: "Ctrl+Shift+F", label: "Fetch all remotes" },
      { keys: "Ctrl+Shift+L", label: "Pull current branch" },
      { keys: "Ctrl+Shift+P", label: "Push current branch" },
      { keys: "Ctrl+B", label: "Create branch" },
    ],
  },
  {
    category: "History & diff",
    items: [
      { keys: "Ctrl+D", label: "Toggle split / unified diff" },
      { keys: "R / F / D", label: "Reword / fixup / drop commit" },
      { keys: "Shift+E", label: "Interactive rebase plan from commit" },
      { keys: "1 / 2", label: "Resolve conflict with Yours / Theirs" },
      { keys: "Ctrl+C", label: "Copy selected commit hash" },
    ],
  },
  {
    category: "Application",
    items: [
      { keys: "Ctrl+Z", label: "Undo last repository or library action" },
      { keys: "Ctrl+Y / Ctrl+Shift+Z", label: "Redo" },
      { keys: "Ctrl+,", label: "Open settings" },
      { keys: "?", label: "Show keyboard shortcuts" },
    ],
  },
];
