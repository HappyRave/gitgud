import { describe, expect, it } from "vitest";
import { blockBrowserMenu } from "./browserMenu";

function rightClick(el: Element) {
  const event = new MouseEvent("contextmenu", { bubbles: true, cancelable: true });
  el.dispatchEvent(event);
  return event;
}

describe("blockBrowserMenu", () => {
  it("hides the browser menu everywhere but text fields", () => {
    document.body.innerHTML = `
      <div id="row"><span>gitgud</span></div>
      <input id="field" />
      <textarea id="area"></textarea>
      <div contenteditable="true"><b id="rich">x</b></div>`;
    window.addEventListener("contextmenu", blockBrowserMenu);
    try {
      expect(rightClick(document.querySelector("#row span")!).defaultPrevented).toBe(true);
      expect(rightClick(document.getElementById("field")!).defaultPrevented).toBe(false);
      expect(rightClick(document.getElementById("area")!).defaultPrevented).toBe(false);
      expect(rightClick(document.getElementById("rich")!).defaultPrevented).toBe(false);
    } finally {
      window.removeEventListener("contextmenu", blockBrowserMenu);
    }
  });
});
