/** True when focus is in a place where keys type text (spec §5 focus rule). */
export function inTextField(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) {
    return false;
  }
  return target.isContentEditable || target instanceof HTMLTextAreaElement || (target instanceof HTMLInputElement && target.type !== "checkbox" && target.type !== "button");
}

/** Ctrl+1 shows the working copy, Ctrl+3 the op log (spec §5). */
export function paneKey(event: KeyboardEvent): "working" | "oplog" | null {
  if (!event.ctrlKey || event.shiftKey || event.altKey || event.metaKey) {
    return null;
  }
  return event.key === "1" ? "working" : event.key === "3" ? "oplog" : null;
}

/** Ctrl+Shift+F fetches (spec §5); a modifier chord, so it works in text fields too. */
export function isFetchKey(event: KeyboardEvent): boolean {
  return event.ctrlKey && event.shiftKey && !event.altKey && !event.metaKey && event.key.toLowerCase() === "f";
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
