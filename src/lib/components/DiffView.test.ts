import { fireEvent, render, screen, within } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import type { FileDiff } from "$lib/api";
import DiffView from "./DiffView.svelte";

const DIFF: FileDiff = {
  path: "src/a.rs",
  binary: false,
  hunks: [
    {
      oldStart: 1,
      oldLines: 2,
      newStart: 1,
      newLines: 2,
      lines: [
        { kind: "context", old: 1, new: 1, text: "fn main() {" },
        { kind: "removed", old: 2, new: null, text: "    old();" },
        { kind: "added", old: null, new: 2, text: "    new();" },
      ],
    },
    {
      oldStart: 40,
      oldLines: 1,
      newStart: 40,
      newLines: 0,
      lines: [{ kind: "removed", old: 40, new: null, text: "}" }],
    },
  ],
};

const rows = () =>
  within(screen.getByRole("region", { name: "Diff of src/a.rs" }))
    .getAllByRole("row")
    .map((tr) => [
      tr.className.split(" ").find((c) => ["context", "added", "removed", "hunk"].includes(c)),
      tr.textContent!.trim(),
    ]);

describe("DiffView", () => {
  it("shows hunk headers and every line with its numbers and sign", () => {
    render(DiffView, { diff: DIFF });

    expect(rows()).toEqual([
      ["hunk", "@@ -1,2 +1,2 @@"],
      ["context", "11 fn main() {"],
      ["removed", "2-    old();"],
      ["added", "2+    new();"],
      ["hunk", "@@ -40 +40,0 @@"],
      ["removed", "40-}"],
    ]);
  });

  it("puts each action on every hunk's own header row", async () => {
    const stage = vi.fn();
    const discard = vi.fn();
    render(DiffView, {
      diff: DIFF,
      hunkActions: [
        { label: "Discard hunk", run: discard },
        { label: "Stage hunk", run: stage },
      ],
    });

    await fireEvent.click(screen.getByRole("button", { name: "Stage hunk @@ -40 +40,0 @@" }));
    await fireEvent.click(screen.getByRole("button", { name: "Discard hunk @@ -1,2 +1,2 @@" }));

    expect(stage).toHaveBeenCalledWith(DIFF.hunks[1]);
    expect(discard).toHaveBeenCalledWith(DIFF.hunks[0]);
    expect(screen.getAllByRole("button")).toHaveLength(4);
  });

  it("disables hunk actions while busy, and has none without an action", () => {
    const { unmount } = render(DiffView, {
      diff: DIFF,
      busy: true,
      hunkActions: [{ label: "Stage hunk", run: () => {} }],
    });
    for (const b of screen.getAllByRole("button")) {
      expect(b).toBeDisabled();
    }
    unmount();

    render(DiffView, { diff: DIFF });
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
  });

  it("says when a file is binary", () => {
    render(DiffView, { diff: { path: "img.png", binary: true, hunks: [] } });

    expect(screen.getByText("Binary file, no text diff.")).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });

  it("says when only the mode changed", () => {
    render(DiffView, { diff: { path: "run.sh", binary: false, hunks: [] } });

    expect(screen.getByText("No content changes.")).toBeInTheDocument();
  });
});
