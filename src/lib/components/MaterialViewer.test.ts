import { fireEvent, render, screen } from "@testing-library/svelte";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ModelSide } from "$lib/model/open";
import { prefs, shaderBase, viewerChoice } from "$lib/prefs.svelte";
import { mockCommands } from "$lib/test/ipc";
import { material } from "$lib/test/material";
import { SHAPES } from "$lib/viewers/material";
import MaterialViewer from "./MaterialViewer.svelte";

vi.mock("./ModelViewer.svelte", async () => ({ default: (await import("$lib/test/Stub.svelte")).default }));

const HEAD: ModelSide = { label: "HEAD", at: { at: "head" } };
const DISK: ModelSide = { label: "On disk", at: { at: "worktree" } };

beforeEach(() => {
  prefs.viewer = {};
});

describe("MaterialViewer", () => {
  it("shows the ball and only what changed, with a Show all", async () => {
    mockCommands({
      material_view: ({ version }) =>
        version === "head"
          ? material({ _BaseColor: "#FFFFFF", _Metallic: "0" }, [])
          : material({ _BaseColor: "#DFE8FF", _Metallic: "0" }, ["_NORMALMAP"]),
      material_users: () => [],
    });
    render(MaterialViewer, { repoId: 1, path: "Assets/M/hull.mat", before: HEAD, after: DISK });
    expect(await screen.findByText("1 changed")).toBeInTheDocument();
    expect(screen.getByTestId("stub")).toHaveAttribute("data-built", "yes");
    expect(screen.getByText("+_NORMALMAP")).toBeInTheDocument();
    expect(screen.getByText("#DFE8FF").closest("td")).toHaveClass("mod");
    expect(screen.queryByText("Metallic")).toBeNull();
    await fireEvent.click(screen.getByRole("button", { name: "Show all" }));
    expect(screen.getByText("Metallic")).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: "Your mesh" })).toBeDisabled();
  });

  it("previews on a mesh that uses it, first one preselected, and remembers the shape", async () => {
    mockCommands({
      material_view: () => material({}, []),
      material_users: () => ["Assets/Boats/Tug.prefab", "Assets/Art/boat.fbx"],
    });
    render(MaterialViewer, { repoId: 1, path: "Assets/M/hull.mat", before: HEAD, after: DISK });
    await fireEvent.click(await screen.findByRole("radio", { name: "Your mesh" }));
    expect(screen.getByTestId("stub")).toHaveAttribute("data-path", "Assets/Boats/Tug.prefab");
    expect(screen.getByTestId("stub")).toHaveAttribute("data-built", "no");
    expect(viewerChoice("Other.mat", "shape", SHAPES)).toBe("mesh");
    await fireEvent.change(screen.getByLabelText("Mesh that uses this material"), { target: { value: "Assets/Art/boat.fbx" } });
    expect(screen.getByTestId("stub")).toHaveAttribute("data-path", "Assets/Art/boat.fbx");
  });

  it("picks which texture of a custom shader shows as the base map, remembered per shader", async () => {
    const slot = (property: string) => ({ property, path: `T/${property}.png`, key: null, tiling: [1, 1] as [number, number], offset: [0, 0] as [number, number] });
    const mozaic = () => {
      const v = material({ _Drawing: "drawing.png" }, []);
      Object.assign(v.material, { shader: "Mozaic", approximated: true, baseProperty: "_Mask", textures: [slot("_Mask"), slot("_Drawing")] });
      return v;
    };
    mockCommands({ material_view: mozaic, material_users: () => [] });
    render(MaterialViewer, { repoId: 1, path: "M/Mozaic3.mat", before: HEAD, after: DISK });
    const picker = (await screen.findByLabelText("Base map")) as HTMLSelectElement;
    expect([...picker.options].map((o) => o.value)).toEqual(["_Mask", "_Drawing"]);
    expect(picker.value).toBe("_Mask");
    await fireEvent.change(picker, { target: { value: "_Drawing" } });
    expect(shaderBase("Mozaic")).toBe("_Drawing");
    expect((screen.getByLabelText("Base map") as HTMLSelectElement).value).toBe("_Drawing");
  });

  it("offers no picker for a standard shader", async () => {
    mockCommands({ material_view: () => material({}, []), material_users: () => [] });
    render(MaterialViewer, { repoId: 1, path: "M/hull.mat", before: HEAD, after: DISK });
    await screen.findByText("Material · URP/Lit");
    expect(screen.queryByLabelText("Base map")).toBeNull();
  });

  it("says why a material can't be read", async () => {
    mockCommands({
      material_view: () => {
        throw new Error("Assets/M/hull.mat has no material in that version");
      },
      material_users: () => [],
    });
    render(MaterialViewer, { repoId: 1, path: "Assets/M/hull.mat", before: null, after: DISK });
    expect(await screen.findByRole("alert")).toHaveTextContent("has no material");
  });
});
