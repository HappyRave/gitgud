import { describe, expect, it, vi } from "vitest";
import { loadMeshBundle } from "./fbxPool";
import { inlineBatcher } from "./inlineBatch";

vi.mock("./fbxPool", () => ({
  loadMeshBundle: vi.fn(async (url: string) =>
    new URL(url).searchParams
      .get("meshes")!
      .split(",")
      .map((id) =>
        id === "bad"
          ? { id, error: "Assets/Room.unity › Mesh &bad: no vertices" }
          : { id, packed: { scale: 1, clips: [], meshes: [{ name: `m${id}`, originalName: null, transformData: null, attributes: [], index: null, groups: [], materials: [] }] } },
      ),
  ),
}));

describe("inlineBatcher", () => {
  it("asks for a task's meshes together, per file, in order, 32 a request", async () => {
    vi.mocked(loadMeshBundle).mockClear();
    const ask = inlineBatcher((path, ids) => `http://x/model?path=${path}&meshes=${ids.join(",")}`);
    const ids = Array.from({ length: 40 }, (_, i) => String(i));
    const room = ids.map((id) => ask("Room", id));
    const hall = ask("Hall", "7");
    const bad = ask("Room", "bad");

    const first = await room[0];
    expect(first.root.getObjectByName("m0")).toBeTruthy();
    await Promise.all([...room, hall]);
    await expect(bad).rejects.toThrow("no vertices");
    expect(vi.mocked(loadMeshBundle).mock.calls.map(([u]) => u)).toEqual([
      `http://x/model?path=Room&meshes=${ids.slice(0, 32).join(",")}`,
      `http://x/model?path=Room&meshes=${[...ids.slice(32), "bad"].join(",")}`,
      "http://x/model?path=Hall&meshes=7",
    ]);
  });
});
