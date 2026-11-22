import { describe, expect, it } from "vitest";
import { isFetchKey, opLogKey, paneKey } from "./keys";

describe("paneKey", () => {
  it("maps Ctrl+1 and Ctrl+3", () => {
    const key = (init: KeyboardEventInit) => paneKey(new KeyboardEvent("keydown", init));
    expect(key({ key: "1", ctrlKey: true })).toBe("working");
    expect(key({ key: "3", ctrlKey: true })).toBe("oplog");
    expect(key({ key: "2", ctrlKey: true })).toBeNull();
    expect(key({ key: "3" })).toBeNull();
    expect(key({ key: "3", ctrlKey: true, shiftKey: true })).toBeNull();
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
