/** True when focus is in a place where keys type text (spec §5 focus rule). */
export function inTextField(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) {
    return false;
  }
  return target.isContentEditable || target instanceof HTMLTextAreaElement || (target instanceof HTMLInputElement && target.type !== "checkbox" && target.type !== "button");
}

/** Ctrl+1 shows the working copy, Ctrl+2 focuses the graph, Ctrl+3 the op log (spec §5). */
export function paneKey(event: KeyboardEvent): "working" | "graph" | "oplog" | null {
  if (!event.ctrlKey || event.shiftKey || event.altKey || event.metaKey) {
    return null;
  }
  return ({ "1": "working", "2": "graph", "3": "oplog" } as const)[event.key as "1" | "2" | "3"] ?? null;
}

export type WorkingAction =
  | "up"
  | "down"
  | "extendUp"
  | "extendDown"
  | "toggle"
  | "discardFile"
  | "stageAll"
  | "message"
  | "nextHunk"
  | "prevHunk"
  | "stageHunk"
  | "unstageHunk"
  | "discardHunk";

/**
 * Working-copy keys (spec §5): ↑↓ files, Shift+↑↓ extend selection, Alt+↑↓ hunks,
 * Space stage / unstage, S / U stage / unstage the hunk, ⌫ discard the hunk,
 * Shift+⌫ the file, A stage all, C the commit message. Off in text fields.
 */
export function workingKey(event: KeyboardEvent): WorkingAction | null {
  if (event.ctrlKey || event.metaKey || inTextField(event.target)) {
    return null;
  }
  const { key, altKey: alt, shiftKey: shift } = event;
  if (alt) {
    return shift ? null : key === "ArrowDown" ? "nextHunk" : key === "ArrowUp" ? "prevHunk" : null;
  }
  if (key === "Backspace") {
    return shift ? "discardFile" : "discardHunk";
  }
  if (shift && (key === "ArrowDown" || key === "ArrowUp")) {
    return key === "ArrowDown" ? "extendDown" : "extendUp";
  }
  if (shift) {
    return null;
  }
  const map: Record<string, WorkingAction> = {
    ArrowDown: "down",
    ArrowUp: "up",
    " ": "toggle",
    s: "stageHunk",
    u: "unstageHunk",
    a: "stageAll",
    c: "message",
  };
  return map[key.length === 1 ? key.toLowerCase() : key] ?? null;
}

/** Ctrl+D switches diffs between unified and side by side (spec §5). */
export function isSplitKey(event: KeyboardEvent): boolean {
  return event.ctrlKey && !event.shiftKey && !event.altKey && !event.metaKey && event.key.toLowerCase() === "d";
}

/** Ctrl+F finds in the history (spec §5). */
export function isFindKey(event: KeyboardEvent): boolean {
  return event.ctrlKey && !event.shiftKey && !event.altKey && !event.metaKey && event.key.toLowerCase() === "f";
}

/** Ctrl+Shift+F fetches (spec §5); a modifier chord, so it works in text fields too. */
export function isFetchKey(event: KeyboardEvent): boolean {
  return event.ctrlKey && event.shiftKey && !event.altKey && !event.metaKey && event.key.toLowerCase() === "f";
}

/** Ctrl+Shift+L pulls (spec §5). */
export function isPullKey(event: KeyboardEvent): boolean {
  return event.ctrlKey && event.shiftKey && !event.altKey && !event.metaKey && event.key.toLowerCase() === "l";
}

/** Ctrl+Shift+P pushes (spec §5). */
export function isPushKey(event: KeyboardEvent): boolean {
  return event.ctrlKey && event.shiftKey && !event.altKey && !event.metaKey && event.key.toLowerCase() === "p";
}

/** Ctrl+B creates a branch at the selected commit or HEAD (spec §5). */
export function isNewBranchKey(event: KeyboardEvent): boolean {
  return event.ctrlKey && !event.shiftKey && !event.altKey && !event.metaKey && event.key.toLowerCase() === "b" && !inTextField(event.target);
}

export type ConflictAction = "yours" | "theirs" | "both" | "edit" | "next" | "prev" | "resolve";

/**
 * Text-merge keys (spec §5): Alt+← / Alt+→ / Alt+↓ use yours / theirs / both,
 * E edits, F7 / Shift+F7 next / previous conflict, Ctrl+Enter marks the file resolved.
 * Off in text fields, where the editor handles its own keys.
 */
export function conflictKey(event: KeyboardEvent): ConflictAction | null {
  if (event.metaKey || inTextField(event.target)) {
    return null;
  }
  const { key, ctrlKey: ctrl, altKey: alt, shiftKey: shift } = event;
  if (alt && !ctrl && !shift) {
    return key === "ArrowLeft" ? "yours" : key === "ArrowRight" ? "theirs" : key === "ArrowDown" ? "both" : null;
  }
  if (key === "F7" && !ctrl && !alt) {
    return shift ? "prev" : "next";
  }
  if (key === "Enter" && ctrl && !alt && !shift) {
    return "resolve";
  }
  if (key.toLowerCase() === "e" && !ctrl && !alt && !shift) {
    return "edit";
  }
  return null;
}

export type PlanKey =
  | { kind: "action"; action: "pick" | "reword" | "edit" | "squash" | "fixup" | "drop" }
  | { kind: "move"; by: -1 | 1 }
  | { kind: "select"; by: -1 | 1 }
  | { kind: "start" }
  | { kind: "cancel" };

/**
 * History-plan keys (spec §5): P R E S F D set the action, Alt+↑↓ moves the row,
 * ↑↓ selects, Ctrl+Enter starts, Esc cancels. Only Ctrl+Enter and Esc work in a
 * text field (the inline message), where Esc leaves the field first.
 */
export function planKey(event: KeyboardEvent): PlanKey | null {
  const { key, ctrlKey: ctrl, altKey: alt, shiftKey: shift, metaKey: meta } = event;
  if (meta) {
    return null;
  }
  if (key === "Enter" && ctrl && !alt && !shift) {
    return { kind: "start" };
  }
  if (inTextField(event.target)) {
    return null;
  }
  if (key === "Escape" && !ctrl && !alt) {
    return { kind: "cancel" };
  }
  const by = key === "ArrowUp" ? -1 : key === "ArrowDown" ? 1 : 0;
  if (by && !ctrl && !shift) {
    return { kind: alt ? "move" : "select", by };
  }
  if (ctrl || alt || shift) {
    return null;
  }
  const action = ({ p: "pick", r: "reword", e: "edit", s: "squash", f: "fixup", d: "drop" } as const)[key.toLowerCase() as "p"];
  return action ? { kind: "action", action } : null;
}

/** Graph quick edits (spec §3.4, §5): R reword, F fixup into parent, D drop, Shift+E edit history from here. */
export function graphEditKey(event: KeyboardEvent): "reword" | "fixup" | "drop" | "plan" | null {
  const { key, ctrlKey: ctrl, altKey: alt, shiftKey: shift, metaKey: meta } = event;
  if (ctrl || alt || meta || inTextField(event.target)) {
    return null;
  }
  const k = key.toLowerCase();
  if (shift) {
    return k === "e" ? "plan" : null;
  }
  return ({ r: "reword", f: "fixup", d: "drop" } as const)[k as "r"] ?? null;
}

/** Side-pick keys: 1 keeps yours, 2 takes theirs (spec §5). */
export function sideKey(event: KeyboardEvent): "yours" | "theirs" | null {
  if (event.ctrlKey || event.altKey || event.metaKey || inTextField(event.target)) {
    return null;
  }
  return event.key === "1" ? "yours" : event.key === "2" ? "theirs" : null;
}

/**
 * The op-log action a key press asks for: Ctrl+Z undo, Ctrl+Y or Ctrl+Shift+Z
 * redo. Inside a text field these stay the field's own text undo and redo.
 */
export function opLogKey(event: KeyboardEvent): "undo" | "redo" | null {
  if (!event.ctrlKey || event.altKey || event.metaKey || inTextField(event.target)) {
    return null;
  }
  const key = event.key.toLowerCase();
  if (key === "z") {
    return event.shiftKey ? "redo" : "undo";
  }
  if (key === "y" && !event.shiftKey) {
    return "redo";
  }
  return null;
}

/** Ctrl+P opens the repository quick-switcher (spec §5). */
export function isQuickSwitchKey(event: KeyboardEvent): boolean {
  return event.ctrlKey && !event.shiftKey && !event.altKey && !event.metaKey && event.key.toLowerCase() === "p";
}

/** Ctrl+W closes the active repository tab (spec §5). */
export function isCloseTabKey(event: KeyboardEvent): boolean {
  return event.ctrlKey && !event.shiftKey && !event.altKey && !event.metaKey && event.key.toLowerCase() === "w";
}

/** Ctrl+Tab / Ctrl+Shift+Tab cycles open tabs (spec §5). */
export function tabCycleKey(event: KeyboardEvent): -1 | 1 | null {
  if (!event.ctrlKey || event.altKey || event.metaKey || event.key !== "Tab") {
    return null;
  }
  return event.shiftKey ? -1 : 1;
}

/** Ctrl+K opens the command palette (spec §5). */
export function isCommandPaletteKey(event: KeyboardEvent): boolean {
  return event.ctrlKey && !event.shiftKey && !event.altKey && !event.metaKey && event.key.toLowerCase() === "k";
}

/** Ctrl+, opens Settings (spec §5). */
export function isSettingsKey(event: KeyboardEvent): boolean {
  return event.ctrlKey && !event.shiftKey && !event.altKey && !event.metaKey && event.key === ",";
}

/** ? outside text fields opens the keyboard shortcut sheet (spec §5). */
export function isShortcutSheetKey(event: KeyboardEvent): boolean {
  return !event.ctrlKey && !event.altKey && !event.metaKey && event.key === "?" && !inTextField(event.target);
}
