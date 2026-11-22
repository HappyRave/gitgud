import { fireEvent, render, screen, within } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import type { ConflictText } from "$lib/api";
import { mockCommands } from "$lib/test/ipc";
import TextMerge from "./TextMerge.svelte";

const long = (prefix: string, n: number) => Array.from({ length: n }, (_, i) => `${prefix}${i + 1}`);

const TEXT: ConflictText = {
  finalNewline: true,
  chunks: [
    { kind: "same", lines: long("top", 10) },
    { kind: "conflict", yours: ["y1"], base: ["b1"], theirs: ["t1"] },
    { kind: "same", lines: ["mid"] },
    { kind: "conflict", yours: ["y2"], base: ["b2"], theirs: [] },
  ],
};

function setup(props: { unity?: boolean; text?: ConflictText | (() => never) } = {}) {
  const text = props.text ?? TEXT;
  const calls = mockCommands({ conflict_text: typeof text === "function" ? text : () => text });
  const onsave = vi.fn();
  const onunmergeable = vi.fn();
  render(TextMerge, {
    repoId: 2,
    path: "src/a.cs",
    revision: 0,
    unity: props.unity,
    yoursLabel: "main",
    theirsLabel: "other",
    onsave,
    onunmergeable,
  });
  return { calls, onsave, onunmergeable };
}

const key = (init: KeyboardEventInit) => fireEvent.keyDown(window, init);
const save = () => screen.getByRole("button", { name: /Save and mark resolved/ });

describe("TextMerge", () => {
  it("shows yours | result | theirs with the branch names and the first conflict current", async () => {
    const { calls } = setup();

    expect(await screen.findByText("Conflict 1 of 2")).toBeInTheDocument();
    expect(calls).toEqual([{ cmd: "conflict_text", args: { id: 2, path: "src/a.cs" } }]);
    expect(screen.getByText("Yours · main")).toBeInTheDocument();
    expect(screen.getByText("Theirs · other")).toBeInTheDocument();
    expect(screen.getAllByText("Pick a side, or")).toHaveLength(2);
    expect(save()).toBeDisabled();
  });

  it("folds long unchanged runs, keeping 3 lines next to the conflict; a click unfolds", async () => {
    setup();
    await screen.findByText("Conflict 1 of 2");

    expect(screen.queryAllByText("top1")).toHaveLength(0);
    expect(screen.getAllByText("top8")).toHaveLength(3);
    await fireEvent.click(screen.getAllByRole("button", { name: "⋯ 7 unchanged lines" })[0]);

    expect(screen.getAllByText("top1")).toHaveLength(3);
  });

  it("Alt+← / Alt+→ pick per hunk, move to the next open one, and Ctrl+Enter saves", async () => {
    const { onsave } = setup();
    await screen.findByText("Conflict 1 of 2");

    await key({ key: "ArrowRight", altKey: true });
    expect(screen.getByText("Conflict 2 of 2")).toBeInTheDocument();
    await key({ key: "ArrowLeft", altKey: true });
    expect(save()).toBeEnabled();
    await key({ key: "Enter", ctrlKey: true });

    expect(onsave).toHaveBeenCalledWith(`${long("top", 10).join("\n")}\nt1\nmid\ny2\n`, "1 yours, 1 theirs");
  });

  it("the arrow in a column takes that side for its hunk", async () => {
    const { onsave } = setup();
    await screen.findByText("Conflict 1 of 2");

    const arrows = screen.getAllByRole("button", { name: "Use theirs" });
    await fireEvent.click(arrows[0]);
    await fireEvent.click(arrows[1]);
    await fireEvent.click(save());

    expect(onsave).toHaveBeenCalledWith(`${long("top", 10).join("\n")}\nt1\nmid\n`, "take theirs");
  });

  it("use both puts yours first; F7 / Shift+F7 move between hunks", async () => {
    const { onsave } = setup();
    await screen.findByText("Conflict 1 of 2");

    await key({ key: "F7" });
    expect(screen.getByText("Conflict 2 of 2")).toBeInTheDocument();
    await key({ key: "F7", shiftKey: true });
    expect(screen.getByText("Conflict 1 of 2")).toBeInTheDocument();
    await fireEvent.click(screen.getAllByRole("button", { name: "use both" })[0]);
    await key({ key: "ArrowDown", altKey: true });
    await fireEvent.click(save());

    expect(onsave).toHaveBeenCalledWith(`${long("top", 10).join("\n")}\ny1\nt1\nmid\ny2\n`, "use both");
  });

  it("E edits the hunk's result; Ctrl+Enter in the editor finishes it, Esc cancels", async () => {
    const { onsave } = setup();
    await screen.findByText("Conflict 1 of 2");

    await key({ key: "e" });
    const editor = screen.getByRole("textbox", { name: "Edit result" });
    expect(editor).toHaveValue("y1\nt1");
    await fireEvent.keyDown(editor, { key: "Escape" });
    expect(screen.queryByRole("textbox")).toBeNull();

    await key({ key: "e" });
    await fireEvent.input(screen.getByRole("textbox", { name: "Edit result" }), { target: { value: "merged by hand" } });
    await fireEvent.keyDown(screen.getByRole("textbox", { name: "Edit result" }), { key: "Enter", ctrlKey: true });
    expect(screen.getByText("Conflict 2 of 2")).toBeInTheDocument();
    await key({ key: "ArrowLeft", altKey: true });
    await fireEvent.click(save());

    expect(onsave).toHaveBeenCalledWith(`${long("top", 10).join("\n")}\nmerged by hand\nmid\ny2\n`, "1 yours, 1 edited");
  });

  it("keys typed in the editor don't pick", async () => {
    setup();
    await screen.findByText("Conflict 1 of 2");
    await key({ key: "e" });
    const editor = screen.getByRole("textbox", { name: "Edit result" });

    await fireEvent.keyDown(editor, { key: "ArrowRight", altKey: true });

    expect(screen.getByText("Conflict 1 of 2")).toBeInTheDocument();
  });

  it("notes that a Unity file merges as text", async () => {
    setup({ unity: true });
    await screen.findByText("Conflict 1 of 2");

    expect(screen.getByText(/Smart Merge isn't set up yet/)).toBeInTheDocument();
  });

  it("hands over to the side picker when Rust says the file can't merge as text", async () => {
    const { onunmergeable } = setup({
      text: () => {
        throw "t.bin is binary: pick a side";
      },
    });

    await vi.waitFor(() => expect(onunmergeable).toHaveBeenCalled());
  });

  it("shows other errors", async () => {
    setup({
      text: () => {
        throw "index locked";
      },
    });

    expect(await screen.findByRole("alert")).toHaveTextContent("index locked");
  });

  it("marks one-sided deletions", async () => {
    setup();
    await screen.findByText("Conflict 1 of 2");
    const region = screen.getByRole("region", { name: "Merge src/a.cs" });

    expect(within(region).getByText("deleted these lines")).toBeInTheDocument();
  });
});
