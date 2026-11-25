import { afterEach, describe, expect, it, vi } from "vitest";
import { fetchGeometry } from "./geometryCache";
import { readPacked } from "./packedBytes";

const FBX = `; FBX 7.3.0 project file
FBXHeaderExtension:  {
	FBXHeaderVersion: 1003
	FBXVersion: 7300
}
Objects:  {
	Geometry: 100, "Geometry::Tri", "Mesh" {
		Vertices: *9 {
			a: 0,0,0,1,0,0,0,1,0
		}
		PolygonVertexIndex: *3 {
			a: 0,1,-3
		}
	}
	Model: 300, "Model::Tri", "Mesh" {
		Version: 232
	}
}
Connections:  {
	C: "OO",300,0
	C: "OO",100,300
}
`;

/** A fake protocol: the model file, and a `/geometry` store that starts empty. */
function serve() {
  const kept = new Map<string, ArrayBuffer>();
  const calls: string[] = [];
  vi.stubGlobal(
    "fetch",
    vi.fn(async (url: string, init?: RequestInit) => {
      calls.push(`${init?.method ?? "GET"} ${url}`);
      if (url.startsWith("geo:")) {
        if (init?.method === "POST") {
          kept.set(url, init.body as ArrayBuffer);
          return new Response("");
        }
        const hit = kept.get(url);
        return hit ? new Response(hit) : new Response("no meshes kept", { status: 404 });
      }
      return new Response(FBX);
    }),
  );
  return { kept, calls };
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("fetchGeometry", () => {
  it("parses on a miss and keeps the result, then uses it without fetching the model again", async () => {
    const { kept, calls } = serve();
    const first = await fetchGeometry("m://tri.fbx", "fbx", "geo:k");
    expect(first.packed.meshes.map((m) => m.name)).toEqual(["Tri"]);
    await vi.waitFor(() => expect(kept.has("geo:k")).toBe(true));
    expect(readPacked(kept.get("geo:k")!).meshes[0].name).toBe("Tri");

    calls.length = 0;
    const again = await fetchGeometry("m://tri.fbx", "fbx", "geo:k");
    expect(calls).toEqual(["GET geo:k"]);
    expect(again.packed.meshes[0].attributes[0].array).toEqual(first.packed.meshes[0].attributes[0].array);
    expect(again.transfer).toHaveLength(1);
  });

  it("hands a fresh parse to `keep` instead of sending it at once", async () => {
    const { kept, calls } = serve();
    const keep = vi.fn();
    await fetchGeometry("m://tri.fbx", "fbx", "geo:k", keep);
    expect(calls).toEqual(["GET geo:k", "GET m://tri.fbx"]);
    expect(keep).toHaveBeenCalledWith("geo:k", expect.any(ArrayBuffer));
    expect(readPacked(keep.mock.calls[0][1]).meshes[0].name).toBe("Tri");
    expect(kept.size).toBe(0);
  });

  it("parses without keeping anything when there is no cache URL", async () => {
    const { kept, calls } = serve();
    await fetchGeometry("m://tri.fbx", "fbx");
    expect(calls).toEqual(["GET m://tri.fbx"]);
    expect(kept.size).toBe(0);
  });
});
