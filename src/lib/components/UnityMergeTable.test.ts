import { fireEvent, render, screen, within } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import type { MergeRow, PropertyMerge } from "$lib/api";
import { mockCommands } from "$lib/test/ipc";
import UnityMergeTable from "./UnityMergeTable.svelte";

vi.mock("./ModelViewer.svelte", async () => ({ default: (await import("$lib/test/Stub.svelte")).default }));

function row(over: Partial<MergeRow> & Pick<MergeRow, "id" | "source">): MergeRow {
  return {
    kind: "value",
    fileId: over.id.split("/")[0],
    objectPath: "Ship / Engine",
    component: "Thruster.cs",
    property: "speed",
    base: "10",
    yours: "15",
    theirs: "22",
    yoursRaw: " 15",
    theirsRaw: " 22",
    editable: true,
    yoursDetail: [],
    theirsDetail: [],
    ...over,
  };
}

const MERGE: PropertyMerge = {
  path: "Assets/Level.unity",
  yours: 1,
  theirs: 1,
  clashes: 2,
  objects: 1,
  rows: [
    row({ id: "12/speed", source: "clash" }),
    row({
      id: "obj:yours:7",
      source: "clash",
      kind: "object",
      objectPath: "OldPier",
      component: "GameObject",
      property: "(object)",
      base: "1 object, 2 components",
      yours: "deleted",
      theirs: "kept, 1 change",
      editable: false,
      theirsDetail: ["Light › m_Range: 12 → 18"],
    }),
    row({ id: "11/m_LocalPosition.x", source: "theirs", objectPath: "Ship", component: "Transform", property: "m_LocalPosition.x", base: "0", yours: "0", theirs: "4", yoursRaw: " 0", theirsRaw: " 4" }),
    row({ id: "13/hp", source: "yours", property: "hp", base: "50", yours: "80", theirs: "50" }),
  ],
};

function setup(merge: PropertyMerge = MERGE) {
  const calls = mockCommands({
    property_merge: () => merge,
    property_merge_check: () => ({
      unresolved: 0,
      issues: [{ path: merge.path, kind: "missingRef", fileId: "4", detail: "Buoys references a missing child" }],
    }),
    property_merge_draft: () => ({ unresolved: 0, issues: [] }),
  });
  const onresolve = vi.fn();
  const onback = vi.fn();
  render(UnityMergeTable, {
    repoId: 1,
    path: merge.path,
    revision: 0,
    yoursLabel: "main",
    theirsLabel: "lighting",
    onresolve,
    onback,
    onfallbacktext: vi.fn(),
  });
  return { calls, onresolve, onback };
}

const rowOf = (text: string) => screen.getByText(text).closest("tr") as HTMLElement;

describe("UnityMergeTable (R9 M2)", () => {
  it("applies one-sided changes, waits for every clash, then saves only the choices made", async () => {
    const { onresolve } = setup();
    await screen.findByText("Clashes · 2 to pick");
    expect(screen.getByText("Changed on one side · applied · 2")).toBeInTheDocument();
    expect(within(rowOf("m_LocalPosition.x")).getByText("theirs")).toBeInTheDocument();
    const save = screen.getByRole("button", { name: /Save & mark resolved/ });
    expect(save).toBeDisabled();

    // The first clash is current: 2 takes theirs and moves to the next clash.
    await fireEvent.keyDown(window, { key: "2" });
    expect(screen.getByText("Clashes · 1 to pick")).toBeInTheDocument();
    expect(screen.getByText("Light › m_Range: 12 → 18")).toBeInTheDocument();
    await fireEvent.keyDown(window, { key: "1" });
    expect(save).toBeEnabled();

    // A one-sided change can be turned back.
    await fireEvent.click(within(rowOf("hp")).getByRole("button", { name: "lighting: 50" }));

    // E edits a value.
    await fireEvent.click(within(rowOf("m_LocalPosition.x")).getByRole("button", { name: "Edit m_LocalPosition.x" }));
    const box = screen.getByLabelText("Edit m_LocalPosition.x");
    expect(box).toHaveValue("4");
    await fireEvent.input(box, { target: { value: "5" } });
    await fireEvent.keyDown(box, { key: "Enter" });

    expect(await screen.findByText(/1 issue in the result/)).toBeInTheDocument();
    await fireEvent.keyDown(window, { key: "Enter", ctrlKey: true });
    expect(onresolve).toHaveBeenCalledWith([
      { id: "12/speed", choice: "theirs", value: null },
      { id: "obj:yours:7", choice: "yours", value: null },
      { id: "11/m_LocalPosition.x", choice: "custom", value: "5" },
      { id: "13/hp", choice: "theirs", value: null },
    ]);
  });

  it("filters rows by source", async () => {
    setup();
    await screen.findByText("Clashes · 2 to pick");
    await fireEvent.click(screen.getByRole("button", { name: /Theirs 1/ }));
    expect(screen.queryByText("hp")).toBeNull();
    expect(screen.getByText("m_LocalPosition.x")).toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: /Objects added \/ deleted 1/ }));
    expect(screen.getByText("OldPier")).toBeInTheDocument();
    expect(screen.queryByText("m_LocalPosition.x")).toBeNull();
  });

  it("asks for a list's order with both readings", async () => {
    setup({
      ...MERGE,
      clashes: 1,
      rows: [
        row({
          id: "4/m_Children#order",
          source: "clash",
          kind: "order",
          objectPath: "Buoys",
          component: "Transform",
          property: "m_Children (order)",
          base: null,
          yours: "yours first",
          theirs: "theirs first",
          editable: false,
          yoursDetail: ["Buoy (4)", "Buoy_Red"],
          theirsDetail: ["Buoy_Red", "Buoy (4)"],
        }),
      ],
    });
    expect(await screen.findByText(/which comes first/)).toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: /Theirs first/ }));
    expect(screen.getByRole("button", { name: /Save & mark resolved/ })).toBeEnabled();
  });

  it("picks every clash to one side at once", async () => {
    const { onresolve } = setup();
    await screen.findByText("Clashes · 2 to pick");
    await fireEvent.click(screen.getByRole("button", { name: "Pick all lighting" }));
    await fireEvent.click(screen.getByRole("button", { name: /Save & mark resolved/ }));
    expect(onresolve).toHaveBeenCalledWith([
      { id: "12/speed", choice: "theirs", value: null },
      { id: "obj:yours:7", choice: "theirs", value: null },
    ]);
  });

  it("builds the 3D result on demand and says when picks have made it out of date", async () => {
    const { calls } = setup();
    await screen.findByText("Clashes · 2 to pick");
    const drafts = () => calls.filter((c) => c.cmd === "property_merge_draft");
    expect(drafts()).toHaveLength(0);

    await fireEvent.click(screen.getByRole("button", { name: "Result in 3D" }));
    const viewer = await screen.findByTestId("stub");
    expect(viewer).toHaveAttribute("data-path", "Assets/Level.unity");
    expect(drafts()).toEqual([{ cmd: "property_merge_draft", args: { id: 1, path: "Assets/Level.unity", picks: [] } }]);
    expect(screen.getByText(/Result · built/)).toBeInTheDocument();

    // Picking doesn't rebuild; it marks the result out of date.
    await fireEvent.keyDown(window, { key: "2" });
    await fireEvent.keyDown(window, { key: "1" });
    expect(screen.getByRole("status")).toHaveTextContent("Out of date · 2 picks since");
    expect(drafts()).toHaveLength(1);

    await fireEvent.keyDown(window, { key: "r" });
    await vi.waitFor(() => expect(screen.queryByRole("status")).toBeNull());
    expect(drafts()[1].args).toMatchObject({
      picks: [
        { id: "12/speed", choice: "theirs", value: null },
        { id: "obj:yours:7", choice: "yours", value: null },
      ],
    });
  });

  it("offers no 3D result for files without one", async () => {
    setup({ ...MERGE, path: "Assets/Ship.asset" });
    await screen.findByText("Clashes · 2 to pick");
    expect(screen.queryByRole("button", { name: "Result in 3D" })).toBeNull();
  });

  it("goes back to picking a side", async () => {
    const { onback } = setup();
    await screen.findByText("Clashes · 2 to pick");
    await fireEvent.click(screen.getByRole("button", { name: "Back to pick a side" }));
    expect(onback).toHaveBeenCalled();
  });

  it("falls back when the file can't be merged by property", async () => {
    mockCommands({
      property_merge: () => {
        throw "Assets/Ship.prefab: can't merge yours by property: line 4: unexpected indentation";
      },
    });
    const onfallbacktext = vi.fn();
    render(UnityMergeTable, {
      repoId: 1,
      path: "Assets/Ship.prefab",
      revision: 0,
      yoursLabel: "main",
      theirsLabel: "feature",
      onresolve: vi.fn(),
      onrawyaml: vi.fn(),
      onfallbacktext,
    });
    await vi.waitFor(() => expect(onfallbacktext).toHaveBeenCalled());
  });
});
