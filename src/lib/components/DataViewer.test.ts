import { fireEvent, render, screen } from "@testing-library/svelte";
import { beforeEach, describe, expect, it } from "vitest";
import type { ModelSide } from "$lib/model/open";
import { prefs, viewerChoice } from "$lib/prefs.svelte";
import { mockCommands } from "$lib/test/ipc";
import { row, tableDiff } from "$lib/test/table";
import { ROW_FILTERS } from "$lib/viewers/table";
import DataViewer from "./DataViewer.svelte";

const HEAD: ModelSide = { label: "HEAD", at: { at: "head" } };
const DISK: ModelSide = { label: "On disk", at: { at: "worktree" } };

beforeEach(() => {
  prefs.viewer = {};
});

describe("DataViewer", () => {
  it("shows changed rows with old → new cells and hides the rest until All rows", async () => {
    const calls = mockCommands({
      data_tables: () => ({
        kind: "CSV",
        compared: true,
        tables: [
          tableDiff(["id", "speed"], [row("changed", "skiff", "12→13.5"), row("same", "tug", "7"), row("added", "foil", "22"), row("removed", "raft", "4")]),
        ],
      }),
    });
    render(DataViewer, { repoId: 1, path: "Data/BoatStats.csv", before: HEAD, after: DISK });
    expect(await screen.findByText("CSV · 3 rows")).toBeInTheDocument();
    expect(calls.find((c) => c.cmd === "data_tables")?.args).toEqual({ id: 1, path: "Data/BoatStats.csv", before: "head", after: "worktree" });
    expect(screen.getByText("12", { exact: false }).closest("td")).toHaveClass("mod");
    expect(screen.getByText("foil").closest("tr")).toHaveClass("added");
    expect(screen.getByText("raft").closest("tr")).toHaveClass("removed");
    expect(screen.queryByText("tug")).toBeNull();
    expect(screen.getByText("1 unchanged rows hidden")).toBeInTheDocument();
    await fireEvent.click(screen.getByRole("radio", { name: "All rows" }));
    expect(screen.getByText("tug")).toBeInTheDocument();
    expect(viewerChoice("Other.csv", "rows", ROW_FILTERS)).toBe("all");
  });

  it("offers a tab per table and shows everything for a new file", async () => {
    mockCommands({
      data_tables: () => ({
        kind: "ScriptableObject · BoatDatabase",
        compared: false,
        tables: [tableDiff(["field", "value"], [row("same", "version", "2")]), tableDiff(["id"], [row("same", "skiff")], { name: "boats" })],
      }),
    });
    render(DataViewer, { repoId: 1, path: "Data/Boats.asset", before: null, after: DISK });
    expect(await screen.findByText("version")).toBeInTheDocument();
    expect(screen.queryByRole("radio", { name: "All rows" })).toBeNull();
    await fireEvent.click(screen.getByRole("tab", { name: "boats" }));
    expect(screen.getByText("skiff")).toBeInTheDocument();
  });

  it("says why a file can't be read", async () => {
    mockCommands({
      data_tables: () => {
        throw new Error("Data/x.json: not JSON this viewer can read (EOF while parsing)");
      },
    });
    render(DataViewer, { repoId: 1, path: "Data/x.json", before: null, after: DISK });
    expect(await screen.findByRole("alert")).toHaveTextContent("not JSON");
  });
});
