import type { PackedMesh, PackedModel } from "./packFbx";

type Typed = PackedMesh["attributes"][number]["array"];

const ARRAYS = { Float32Array, Uint32Array, Uint16Array, Int16Array, Uint8Array, Int8Array } as const;
type ArrayKind = keyof typeof ARRAYS;

/** Where one typed array sits in the data after the header. */
interface Span {
  kind: ArrayKind;
  offset: number;
  length: number;
}

const MAGIC = 0x31_47_47_47; // "GGG1"

const kindOf = (a: Typed): ArrayKind => a.constructor.name as ArrayKind;

/**
 * A packed model as one buffer, to keep in gitgud's disk cache: a JSON header (names,
 * pivots, groups, materials, clips and where each array is) then the arrays, 8-byte aligned.
 */
export function writePacked(p: PackedModel): ArrayBuffer {
  const arrays: Typed[] = [];
  let size = 0;
  const span = (a: Typed): Span => {
    size = Math.ceil(size / 8) * 8;
    const s = { kind: kindOf(a), offset: size, length: a.length };
    size += a.byteLength;
    arrays.push(a);
    return s;
  };
  const meshes = p.meshes.map((m) => ({
    ...m,
    attributes: m.attributes.map((a) => ({ ...a, array: span(a.array) })),
    index: m.index ? span(m.index) : null,
  }));
  const header = new TextEncoder().encode(JSON.stringify({ scale: p.scale, clips: p.clips, meshes }));
  const start = Math.ceil((8 + header.byteLength) / 8) * 8;
  const out = new ArrayBuffer(start + size);
  const view = new DataView(out);
  view.setUint32(0, MAGIC, true);
  view.setUint32(4, header.byteLength, true);
  new Uint8Array(out, 8, header.byteLength).set(header);
  const spans = meshes.flatMap((m) => [...m.attributes.map((a) => a.array), ...(m.index ? [m.index] : [])]);
  spans.forEach((s, i) => {
    const a = arrays[i];
    new Uint8Array(out, start + s.offset, a.byteLength).set(new Uint8Array(a.buffer, a.byteOffset, a.byteLength));
  });
  return out;
}

/** Reads what [`writePacked`] wrote; the arrays are views of `buf`, so it is the one buffer to transfer. */
export function readPacked(buf: ArrayBuffer): PackedModel {
  const view = new DataView(buf);
  if (buf.byteLength < 8 || view.getUint32(0, true) !== MAGIC) {
    throw new Error("not a packed model");
  }
  const length = view.getUint32(4, true);
  const header = JSON.parse(new TextDecoder().decode(new Uint8Array(buf, 8, length))) as {
    scale: number;
    clips: PackedModel["clips"];
    meshes: (Omit<PackedMesh, "attributes" | "index"> & {
      attributes: (Omit<PackedMesh["attributes"][number], "array"> & { array: Span })[];
      index: Span | null;
    })[];
  };
  const start = Math.ceil((8 + length) / 8) * 8;
  const array = (s: Span) => new ARRAYS[s.kind](buf, start + s.offset, s.length);
  return {
    scale: header.scale,
    clips: header.clips,
    meshes: header.meshes.map((m) => ({
      ...m,
      attributes: m.attributes.map((a) => ({ ...a, array: array(a.array) as Typed })),
      index: m.index ? (array(m.index) as Uint32Array | Uint16Array) : null,
    })),
  };
}
