import { describe, expect, it } from "vitest";
import {
  conflictKey,
  graphEditKey,
  isFetchKey,
  isFindKey,
  isNewBranchKey,
  isPullKey,
  isPushKey,
  isQuickSwitchKey,
  isCloseTabKey,
  isSplitKey,
  opLogKey,
  paneKey,
  planKey,
  sideKey,
  tabCycleKey,
  workingKey,
} from "./keys";

describe("paneKey", () => {
  it("maps Ctrl+1, Ctrl+2 and Ctrl+3", () => {
    const key = (init: KeyboardEventInit) => paneKey(new KeyboardEvent("keydown", init));
    expect(key({ key: "1", ctrlKey: true })).toBe("working");
    expect(key({ key: "2", ctrlKey: true })).toBe("graph");
    expect(key({ key: "3", ctrlKey: true })).toBe("oplog");
    expect(key({ key: "4", ctrlKey: true })).toBeNull();
    expect(key({ key: "3" })).toBeNull();
    expect(key({ key: "3", ctrlKey: true, shiftKey: true })).toBeNull();
  });
});

describe("isSplitKey", () => {
  it("is Ctrl+D only", () => {
    const key = (init: KeyboardEventInit) => isSplitKey(new KeyboardEvent("keydown", init));
    expect(key({ key: "d", ctrlKey: true })).toBe(true);
    expect(key({ key: "d" })).toBe(false);
    expect(key({ key: "D", ctrlKey: true, shiftKey: true })).toBe(false);
  });
});

describe("isFindKey", () => {
  it("is Ctrl+F without Shift", () => {
    const key = (init: KeyboardEventInit) => isFindKey(new KeyboardEvent("keydown", init));
    expect(key({ key: "f", ctrlKey: true })).toBe(true);
    expect(key({ key: "F", ctrlKey: true, shiftKey: true })).toBe(false);
    expect(key({ key: "f" })).toBe(false);
  });
});

describe("isFetchKey", () => {
  it("is Ctrl+Shift+F only", () => {
    const key = (init: KeyboardEventInit) => isFetchKey(new KeyboardEvent("keydown", init));
    expect(key({ key: "F", ctrlKey: true, shiftKey: true })).toBe(true);
    expect(key({ key: "f", ctrlKey: true })).toBe(false);
    expect(key({ key: "f", ctrlKey: true, shiftKey: true, altKey: true })).toBe(false);
  });
});

function press(init: KeyboardEventInit, target: HTMLElement = document.body) {
  const event = new KeyboardEvent("keydown", { bubbles: true, ...init });
  Object.defineProperty(event, "target", { value: target });
  return opLogKey(event);
}

describe("opLogKey", () => {
  it("maps Ctrl+Z to undo and Ctrl+Y / Ctrl+Shift+Z to redo", () => {
    expect(press({ key: "z", ctrlKey: true })).toBe("undo");
    expect(press({ key: "y", ctrlKey: true })).toBe("redo");
    expect(press({ key: "Z", ctrlKey: true, shiftKey: true })).toBe("redo");
  });

  it("ignores other keys and modifiers", () => {
    expect(press({ key: "z" })).toBeNull();
    expect(press({ key: "z", ctrlKey: true, altKey: true })).toBeNull();
    expect(press({ key: "y", ctrlKey: true, shiftKey: true })).toBeNull();
    expect(press({ key: "x", ctrlKey: true })).toBeNull();
  });

  it("leaves text fields their own undo", () => {
    for (const field of [document.createElement("textarea"), document.createElement("input")]) {
      expect(press({ key: "z", ctrlKey: true }, field)).toBeNull();
    }
    const editable = document.createElement("div");
    editable.contentEditable = "true";
    Object.defineProperty(editable, "isContentEditable", { value: true });
    expect(press({ key: "y", ctrlKey: true }, editable)).toBeNull();
  });

  it("still works from buttons and checkboxes", () => {
    const checkbox = document.createElement("input");
    checkbox.type = "checkbox";
    expect(press({ key: "z", ctrlKey: true }, checkbox)).toBe("undo");
    expect(press({ key: "z", ctrlKey: true }, document.createElement("button"))).toBe("undo");
  });
});

function event(init: KeyboardEventInit, target: HTMLElement = document.body) {
  const e = new KeyboardEvent("keydown", { bubbles: true, ...init });
  Object.defineProperty(e, "target", { value: target });
  return e;
}

describe("conflictKey", () => {
  it("maps the text-merge keys", () => {
    expect(conflictKey(event({ key: "ArrowLeft", altKey: true }))).toBe("yours");
    expect(conflictKey(event({ key: "ArrowRight", altKey: true }))).toBe("theirs");
    expect(conflictKey(event({ key: "ArrowDown", altKey: true }))).toBe("both");
    expect(conflictKey(event({ key: "F7" }))).toBe("next");
    expect(conflictKey(event({ key: "F7", shiftKey: true }))).toBe("prev");
    expect(conflictKey(event({ key: "Enter", ctrlKey: true }))).toBe("resolve");
    expect(conflictKey(event({ key: "e" }))).toBe("edit");
  });

  it("ignores plain arrows, other chords and text fields", () => {
    expect(conflictKey(event({ key: "ArrowLeft" }))).toBeNull();
    expect(conflictKey(event({ key: "ArrowLeft", altKey: true, ctrlKey: true }))).toBeNull();
    expect(conflictKey(event({ key: "Enter" }))).toBeNull();
    expect(conflictKey(event({ key: "e" }, document.createElement("textarea")))).toBeNull();
  });
});

describe("workingKey", () => {
  it("maps the working-copy keys", () => {
    const cases: [KeyboardEventInit, string][] = [
      [{ key: "ArrowDown" }, "down"],
      [{ key: "ArrowUp" }, "up"],
      [{ key: " " }, "toggle"],
      [{ key: "Backspace", shiftKey: true }, "discardFile"],
      [{ key: "Backspace" }, "discardHunk"],
      [{ key: "a" }, "stageAll"],
      [{ key: "C" }, "message"],
      [{ key: "ArrowDown", altKey: true }, "nextHunk"],
      [{ key: "ArrowUp", altKey: true }, "prevHunk"],
      [{ key: "s" }, "stageHunk"],
      [{ key: "u" }, "unstageHunk"],
    ];
    for (const [init, action] of cases) {
      expect(workingKey(event(init))).toBe(action);
    }
  });

  it("ignores Ctrl chords, Shift+letters and text fields", () => {
    expect(workingKey(event({ key: "a", ctrlKey: true }))).toBeNull();
    expect(workingKey(event({ key: "S", shiftKey: true }))).toBeNull();
    expect(workingKey(event({ key: " " }, document.createElement("textarea")))).toBeNull();
  });
});

describe("planKey", () => {
  it("maps the action letters, Alt+↑↓ moves, ↑↓ selects, Ctrl+Enter and Esc", () => {
    expect(planKey(event({ key: "s" }))).toEqual({ kind: "action", action: "squash" });
    expect(planKey(event({ key: "D" }))).toEqual({ kind: "action", action: "drop" });
    expect(planKey(event({ key: "ArrowUp", altKey: true }))).toEqual({ kind: "move", by: -1 });
    expect(planKey(event({ key: "ArrowDown" }))).toEqual({ kind: "select", by: 1 });
    expect(planKey(event({ key: "Enter", ctrlKey: true }))).toEqual({ kind: "start" });
    expect(planKey(event({ key: "Escape" }))).toEqual({ kind: "cancel" });
    expect(planKey(event({ key: "x" }))).toBeNull();
    expect(planKey(event({ key: "s", ctrlKey: true }))).toBeNull();
  });

  it("leaves a text field its keys but Ctrl+Enter", () => {
    const field = document.createElement("textarea");
    expect(planKey(event({ key: "s" }, field))).toBeNull();
    expect(planKey(event({ key: "Escape" }, field))).toBeNull();
    expect(planKey(event({ key: "Enter", ctrlKey: true }, field))).toEqual({ kind: "start" });
  });
});

describe("graphEditKey", () => {
  it("maps R, F, D and Shift+E outside text fields", () => {
    expect(graphEditKey(event({ key: "r" }))).toBe("reword");
    expect(graphEditKey(event({ key: "f" }))).toBe("fixup");
    expect(graphEditKey(event({ key: "d" }))).toBe("drop");
    expect(graphEditKey(event({ key: "E", shiftKey: true }))).toBe("plan");
    expect(graphEditKey(event({ key: "e" }))).toBeNull();
    expect(graphEditKey(event({ key: "r", ctrlKey: true }))).toBeNull();
    expect(graphEditKey(event({ key: "r" }, document.createElement("input")))).toBeNull();
  });
});

describe("sideKey", () => {
  it("maps 1 and 2 outside text fields", () => {
    expect(sideKey(event({ key: "1" }))).toBe("yours");
    expect(sideKey(event({ key: "2" }))).toBe("theirs");
    expect(sideKey(event({ key: "2", ctrlKey: true }))).toBeNull();
    expect(sideKey(event({ key: "1" }, document.createElement("input")))).toBeNull();
  });
});

describe("pull, push, and new-branch keys", () => {
  it("maps Ctrl+Shift+L to pull and Ctrl+Shift+P to push", () => {
    expect(isPullKey(event({ key: "l", ctrlKey: true, shiftKey: true }))).toBe(true);
    expect(isPullKey(event({ key: "l", ctrlKey: true }))).toBe(false);
    expect(isPushKey(event({ key: "P", ctrlKey: true, shiftKey: true }))).toBe(true);
    expect(isPushKey(event({ key: "p", ctrlKey: true }))).toBe(false);
  });

  it("maps Ctrl+B outside text fields to new branch", () => {
    expect(isNewBranchKey(event({ key: "b", ctrlKey: true }))).toBe(true);
    expect(isNewBranchKey(event({ key: "b", ctrlKey: true, shiftKey: true }))).toBe(false);
    expect(isNewBranchKey(event({ key: "b", ctrlKey: true }, document.createElement("textarea")))).toBe(false);
  });

  it("maps Ctrl+P, Ctrl+W, and Ctrl+Tab / Ctrl+Shift+Tab", () => {
    expect(isQuickSwitchKey(event({ key: "p", ctrlKey: true }))).toBe(true);
    expect(isCloseTabKey(event({ key: "w", ctrlKey: true }))).toBe(true);
    expect(tabCycleKey(event({ key: "Tab", ctrlKey: true }))).toBe(1);
    expect(tabCycleKey(event({ key: "Tab", ctrlKey: true, shiftKey: true }))).toBe(-1);
    expect(tabCycleKey(event({ key: "Tab" }))).toBeNull();
  });
});
