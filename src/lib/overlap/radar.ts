import type { Overlap, OverlapFile, Risk } from "$lib/api";
import { age } from "$lib/time";

export const RISK_LABEL: Record<Risk, string> = {
  wontMerge: "won't merge",
  mayConflict: "may conflict",
  merges: "merges",
};

/** The overlap entry for a working-copy row: the file itself, or the asset a `.meta` belongs to. */
export function overlapFor(overlap: Overlap | null, path: string): OverlapFile | null {
  if (!overlap) {
    return null;
  }
  const asset = path.endsWith(".meta") ? path.slice(0, -5) : path;
  return overlap.files.find((f) => f.path === asset) ?? null;
}

/** "JG" for "Jean Guy", "MD" for "MD", "M" for "max". */
export function initials(name: string): string {
  const words = name.trim().split(/[\s._-]+/).filter(Boolean);
  if (words.length === 1 && /^[A-Z]{2,3}$/.test(words[0])) {
    return words[0];
  }
  return words
    .slice(0, 2)
    .map((w) => w[0].toUpperCase())
    .join("");
}

/** One stable colour per author, from the palette the R11 round used. */
const PEOPLE = ["#7fd9a6", "#56b4e9", "#f2c46a", "#ff9d80", "#cdb4ff", "#e8b84a", "#8b95a5"];
export function personColor(name: string): string {
  let h = 0;
  for (const c of name) {
    h = (h * 31 + c.charCodeAt(0)) >>> 0;
  }
  return PEOPLE[h % PEOPLE.length];
}

/** Key of one heads-up: a file and the newest origin commit that changed it. */
export const headsUpKey = (f: OverlapFile) => `${f.path}@${f.newestCommit}`;

/** Won't-merge overlaps not announced yet; the others stay marks only (R11 Q4). */
export function freshHeadsUps(overlap: Overlap | null, seen: ReadonlySet<string>): OverlapFile[] {
  return (overlap?.files ?? []).filter((f) => f.risk === "wontMerge" && !seen.has(headsUpKey(f)));
}

const fileName = (path: string) => path.slice(path.lastIndexOf("/") + 1);

/** "40 min ago", "just now". */
export function ago(seconds: number, now = Date.now() / 1000): string {
  const a = age(seconds, now);
  return a === "now" ? "just now" : /\d (min|h)$/.test(a) ? `${a} ago` : `on ${a}`;
}

export function headline(f: OverlapFile, now = Date.now() / 1000): string {
  return `${fileName(f.path)} changed on ${f.on} ${ago(f.newest, now)}`;
}

/** Why it matters and what to do, for the toast and the notification. */
export function advice(f: OverlapFile): string {
  const by = f.authors.join(", ");
  if (f.trunk) {
    return `${by} changed it on ${f.on} and ${f.dirty ? "you're editing it" : "so does a commit of yours"}. Git can't merge it line by line: merge ${f.on} in now, while your part is small.`;
  }
  return f.dirty
    ? `${by} changed it and you're editing it. Git can't merge it line by line: commit and pull now, while your part is small.`
    : `${by} changed it and so does a commit you haven't pushed. Git can't merge it line by line: pull now, before more piles on.`;
}

/** The strip's count per remote branch (upstream first), worst risk first in each. */
export function stripLabel(overlap: Overlap | null): { text: string; risk: Risk } | null {
  const files = overlap?.files ?? [];
  if (!files.length) {
    return null;
  }
  const many = (k: number) => (k === 1 ? "file" : "files");
  const parts = (overlap?.against ?? []).flatMap((on) => {
    const mine = files.filter((f) => f.on === on);
    const wont = mine.filter((f) => f.risk === "wontMerge").length;
    if (!mine.length) {
      return [];
    }
    return wont ? [`${wont} ${many(wont)} won't merge with ${on}`] : [`${mine.length} ${many(mine.length)} also changed on ${on}`];
  });
  return { text: parts.join(" · "), risk: files[0].risk };
}
