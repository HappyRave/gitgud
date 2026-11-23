const M = (1n << 64n) - 1n;
const P1 = 11400714785074694791n;
const P2 = 14029467366897019727n;
const P3 = 1609587929392839161n;
const P4 = 9650029242287828579n;
const P5 = 2870177450012600261n;

const rotl = (x: bigint, r: bigint) => ((x << r) | (x >> (64n - r))) & M;
const round = (acc: bigint, lane: bigint) => (rotl((acc + lane * P2) & M, 31n) * P1) & M;
const merge = (acc: bigint, v: bigint) => ((acc ^ round(0n, v)) * P1 + P4) & M;

function lane(b: Uint8Array, at: number, bytes: number): bigint {
  let v = 0n;
  for (let i = bytes - 1; i >= 0; i--) {
    v = (v << 8n) | BigInt(b[at + i]);
  }
  return v;
}

/** xxHash64 with seed 0. */
export function xxh64(input: string): bigint {
  const b = new TextEncoder().encode(input);
  const n = b.length;
  let p = 0;
  let h: bigint;
  if (n >= 32) {
    let v1 = (P1 + P2) & M;
    let v2 = P2;
    let v3 = 0n;
    let v4 = (-P1) & M;
    for (; p + 32 <= n; p += 32) {
      v1 = round(v1, lane(b, p, 8));
      v2 = round(v2, lane(b, p + 8, 8));
      v3 = round(v3, lane(b, p + 16, 8));
      v4 = round(v4, lane(b, p + 24, 8));
    }
    h = (rotl(v1, 1n) + rotl(v2, 7n) + rotl(v3, 12n) + rotl(v4, 18n)) & M;
    h = merge(merge(merge(merge(h, v1), v2), v3), v4);
  } else {
    h = P5;
  }
  h = (h + BigInt(n)) & M;
  for (; p + 8 <= n; p += 8) {
    h = ((rotl(h ^ round(0n, lane(b, p, 8)), 27n) * P1) + P4) & M;
  }
  if (p + 4 <= n) {
    h = ((rotl(h ^ ((lane(b, p, 4) * P1) & M), 23n) * P2) + P3) & M;
    p += 4;
  }
  for (; p < n; p++) {
    h = (rotl(h ^ ((BigInt(b[p]) * P5) & M), 11n) * P1) & M;
  }
  h ^= h >> 33n;
  h = (h * P2) & M;
  h ^= h >> 29n;
  h = (h * P3) & M;
  h ^= h >> 32n;
  return h;
}

/** Unity's `fileID` for a model sub-asset named `name` (its `occurrence`-th with that name). */
export function unityFileId(type: string, name: string, occurrence = 0): string {
  return BigInt.asIntN(64, xxh64(`Type:${type}->${name}${occurrence}`)).toString();
}
