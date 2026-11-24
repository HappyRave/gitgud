import { BoxGeometry, Group, Mesh, MeshStandardMaterial, type Object3D } from "three";
import { describe, expect, it } from "vitest";
import { compare, namePaths, stats } from "./compare";

const mat = (name: string) => Object.assign(new MeshStandardMaterial(), { name });

function mesh(name: string, size = 1, material = "M_Grey"): Mesh {
  return Object.assign(new Mesh(new BoxGeometry(size, size, size), mat(material)), { name });
}

function boat(version: "old" | "new"): Group {
  const root = new Group();
  const hull = mesh("Hull", 2, version === "old" ? "M_Hull_Red" : "M_Hull_Navy");
  const cabin = mesh("Cabin", version === "old" ? 1 : 1.2);
  cabin.position.x = version === "old" ? 0 : -0.35;
  cabin.add(mesh("Windshield", 0.2));
  root.add(hull, cabin);
  if (version === "old") {
    const mast = mesh("Mast", 0.1);
    mast.add(mesh("Flag", 0.1));
    root.add(mast);
  } else {
    const rack = new Group();
    rack.name = "RoofRack";
    rack.add(mesh("Bar"), mesh("Bar"));
    cabin.add(rack);
  }
  return root;
}

const byName = (changes: { name: string; kind: string; note: string }[]) =>
  Object.fromEntries(changes.map((c) => [c.name, `${c.kind}: ${c.note}`]));

describe("compare", () => {
  it("names repeated siblings apart", () => {
    expect([...namePaths(boat("new")).keys()]).toEqual([
      "Hull",
      "Cabin",
      "Cabin/Windshield",
      "Cabin/RoofRack",
      "Cabin/RoofRack/Bar",
      "Cabin/RoofRack/Bar#2",
    ]);
  });

  it("lists the topmost added and removed objects and every modified one, with why", () => {
    const before = boat("old");
    const after = boat("new");
    const { status, changes } = compare(before, after);
    expect(byName(changes)).toEqual({
      Hull: "modified: M_Hull_Red → M_Hull_Navy",
      Cabin: "modified: mesh changed · moved 0.35 m",
      RoofRack: "added: with 2 children",
      Mast: "removed: with 1 child",
    });
    const find = (root: Object3D, name: string) => root.getObjectByName(name)!;
    expect(status.get(find(after, "Windshield"))).toBe("modified");
    expect(status.get(find(before, "Windshield"))).toBe("modified");
    expect(status.get(find(after, "Hull"))).toBe("modified");
    expect(status.get(find(after, "Bar"))).toBe("added");
    expect(status.get(find(before, "Flag"))).toBe("removed");
    expect(status.get(find(before, "Cabin"))).toBe("modified");
  });

  it("tints a moved object's mesh child but leaves its unmoved siblings", () => {
    const version = (x: number) => {
      const root = new Group();
      const crane = Object.assign(new Group(), { name: "Crane" });
      crane.position.x = x;
      crane.add(mesh("Crane_Mesh"));
      const pier = Object.assign(new Group(), { name: "Pier" });
      pier.add(mesh("Pier_Mesh"));
      root.add(crane, pier);
      return root;
    };
    const after = version(5);
    const { status, changes } = compare(version(3), after);
    expect(changes.map((c) => c.name)).toEqual(["Crane"]);
    expect(status.get(after.getObjectByName("Crane_Mesh")!)).toBe("modified");
    expect(status.get(after.getObjectByName("Pier_Mesh")!)).toBe("same");
  });

  it("finds nothing between identical versions", () => {
    expect(compare(boat("old"), boat("old")).changes).toEqual([]);
  });

  it("counts meshes, triangles and distinct materials", () => {
    const s = stats(boat("old"));
    expect(s).toMatchObject({ objects: 5, meshes: 5, vertices: 5 * 24, triangles: 5 * 12, materials: 5, textures: 0 });
  });
});
