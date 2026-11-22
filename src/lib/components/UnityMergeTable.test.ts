import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import { mockCommands } from "$lib/test/ipc";
import UnityMergeTable from "./UnityMergeTable.svelte";

describe("UnityMergeTable", () => {
  it("loads R4-A property table, supports 1/2/E picks, and saves property picks", async () => {
    mockCommands({
      unity_conflict_table: () => ({
        path: "Assets/Ship.prefab",
        conflicts: [
          {
            id: "12::speed",
            fileId: "12",
            objectPath: "Ship / Engine",
            component: "Thruster.cs",
            property: "speed",
            base: "10",
            yours: "15",
            theirs: "22",
            baseDisplay: "10",
            yoursDisplay: "15",
            theirsDisplay: "22",
          },
        ],
        autoMerged: [
          {
            fileId: "11",
            objectPath: "Ship",
            component: "Transform",
            property: "m_LocalPosition.x",
            side: "theirs",
            value: "4",
            display: "4",
          },
        ],
      }),
    });
    const onresolve = vi.fn();
    const onrawyaml = vi.fn();

    render(UnityMergeTable, {
      repoId: 1,
      path: "Assets/Ship.prefab",
      revision: 0,
      yoursLabel: "main",
      theirsLabel: "feature",
      onresolve,
      onrawyaml,
      onfallbacktext: vi.fn(),
    });

    await screen.findByText("Ship / Engine");
    expect(screen.getByText(/Auto-merged cleanly \(1 properties\)/)).toBeInTheDocument();

    const saveBtn = screen.getByRole("button", { name: /Save & stage/ });
    expect(saveBtn).toBeDisabled();

    await fireEvent.keyDown(window, { key: "2" });
    expect(saveBtn).toBeEnabled();

    await fireEvent.click(screen.getByRole("button", { name: "Edit" }));
    const editBox = screen.getByLabelText("Edit speed");
    await fireEvent.input(editBox, { target: { value: "30" } });
    await fireEvent.keyDown(editBox, { key: "Enter" });

    await fireEvent.click(saveBtn);
    expect(onresolve).toHaveBeenCalledWith([{ fileId: "12", property: "speed", value: "30" }]);

    await fireEvent.click(screen.getByRole("button", { name: "Raw YAML" }));
    expect(onrawyaml).toHaveBeenCalled();
  });

  it("calls onfallbacktext when file is not parseable Unity YAML", async () => {
    mockCommands({
      unity_conflict_table: () => {
        throw "Assets/Ship.prefab is not valid multi-document Unity YAML";
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
