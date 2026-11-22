import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, it } from "vitest";
import type { UnityYamlDiff } from "$lib/api";
import UnityDiffView from "./UnityDiffView.svelte";

const SAMPLE: UnityYamlDiff = {
  path: "Assets/Prefabs/Ship.prefab",
  addedObjects: 1,
  removedObjects: 0,
  modifiedProperties: 2,
  objects: [
    {
      fileId: "10",
      objectPath: "Ship / Hull",
      component: "Transform",
      kind: "modified",
      properties: [
        {
          property: "m_LocalPosition",
          before: "{x: 0, y: 0, z: 0}",
          after: "{x: 2, y: 0, z: 0}",
          beforeDisplay: "{x: 0, y: 0, z: 0}",
          afterDisplay: "{x: 2, y: 0, z: 0}",
        },
      ],
    },
    {
      fileId: "20",
      objectPath: "Ship / Engine",
      component: "Thruster.cs",
      kind: "added",
      properties: [
        {
          property: "thrust",
          before: null,
          after: "45",
          beforeDisplay: null,
          afterDisplay: "45",
        },
      ],
    },
  ],
};

describe("UnityDiffView", () => {
  it("shows object hierarchy, component script names, and filters properties", async () => {
    render(UnityDiffView, { diff: SAMPLE });

    expect(screen.getByText("Ship / Hull")).toBeInTheDocument();
    expect(screen.getByText(/Thruster\.cs/)).toBeInTheDocument();
    expect(screen.getByText("{x: 2, y: 0, z: 0}")).toBeInTheDocument();

    await fireEvent.input(screen.getByLabelText("Filter objects or properties"), { target: { value: "thrust" } });
    expect(screen.getByText("Ship / Engine")).toBeInTheDocument();
    expect(screen.queryByText("Ship / Hull")).toBeNull();
  });
});
