export type PageState = "same" | "changed" | "added" | "removed";

export interface PageMatch {
  /** 0-based page in the earlier version, or null for an added page. */
  before: number | null;
  after: number | null;
  state: PageState;
  /** Neither page has text (a scan), so it was paired by position, not compared. */
  noText: boolean;
}

const norm = (s: string) => s.replace(/\s+/g, " ").trim();

/**
 * Lines pages up by their text, so an inserted page doesn't make every later page
 * "changed": equal texts anchor, pages between anchors pair up in order as changed,
 * the rest are added or removed. Pages without text never anchor.
 */
export function matchPages(before: string[], after: string[]): PageMatch[] {
  const b = before.map(norm);
  const a = after.map(norm);
  const n = b.length;
  const m = a.length;
  const lcs = Array.from({ length: n + 1 }, () => new Uint32Array(m + 1));
  for (let i = n - 1; i >= 0; i--) {
    for (let j = m - 1; j >= 0; j--) {
      lcs[i][j] = b[i] !== "" && b[i] === a[j] ? lcs[i + 1][j + 1] + 1 : Math.max(lcs[i + 1][j], lcs[i][j + 1]);
    }
  }
  const out: PageMatch[] = [];
  const gap = (bs: number[], as: number[]) => {
    const k = Math.min(bs.length, as.length);
    for (let x = 0; x < k; x++) {
      out.push({ before: bs[x], after: as[x], state: "changed", noText: b[bs[x]] === "" && a[as[x]] === "" });
    }
    for (const i of bs.slice(k)) {
      out.push({ before: i, after: null, state: "removed", noText: false });
    }
    for (const j of as.slice(k)) {
      out.push({ before: null, after: j, state: "added", noText: false });
    }
  };
  let i = 0;
  let j = 0;
  let bs: number[] = [];
  let as: number[] = [];
  while (i < n || j < m) {
    if (i < n && j < m && b[i] !== "" && b[i] === a[j] && lcs[i][j] === lcs[i + 1][j + 1] + 1) {
      gap(bs, as);
      bs = [];
      as = [];
      out.push({ before: i, after: j, state: "same", noText: false });
      i++;
      j++;
    } else if (j < m && (i >= n || lcs[i][j + 1] >= lcs[i + 1][j])) {
      as.push(j++);
    } else {
      bs.push(i++);
    }
  }
  gap(bs, as);
  return out;
}

/**
 * Text runs of one page that the other page doesn't have (as a multiset), by
 * index, for highlighting what changed.
 */
export function changedRuns(mine: string[], theirs: string[]): Set<number> {
  const pool = new Map<string, number>();
  for (const s of theirs) {
    const k = norm(s);
    pool.set(k, (pool.get(k) ?? 0) + 1);
  }
  const out = new Set<number>();
  mine.forEach((s, i) => {
    const k = norm(s);
    if (k === "") {
      return;
    }
    const left = pool.get(k) ?? 0;
    if (left > 0) {
      pool.set(k, left - 1);
    } else {
      out.add(i);
    }
  });
  return out;
}

/** "3", or "7–8" when a page moved: the after page, else the before page. */
export function pageLabel(p: PageMatch): string {
  if (p.after === null) {
    return `${(p.before ?? 0) + 1}`;
  }
  if (p.before !== null && p.before !== p.after) {
    return `${p.before + 1}→${p.after + 1}`;
  }
  return `${p.after + 1}`;
}

/** "12 pages · 2 changed · 1 added (p.5) · pages matched by their text". */
export function pdfCaption(pages: PageMatch[], afterCount: number, compare: boolean): string[] {
  const out = [`${afterCount} ${afterCount === 1 ? "page" : "pages"}`];
  if (!compare) {
    return out;
  }
  const list = (s: PageState, side: "before" | "after") => {
    const ps = pages.filter((p) => p.state === s && !p.noText).map((p) => (p[side] ?? 0) + 1);
    return ps.length ? `${ps.length} ${s} (p.${ps.slice(0, 4).join(", ")}${ps.length > 4 ? "…" : ""})` : null;
  };
  const parts = [list("changed", "after"), list("added", "after"), list("removed", "before")].filter((x): x is string => !!x);
  out.push(...(parts.length ? parts : ["no page changed"]));
  const scans = pages.filter((p) => p.noText).length;
  if (scans) {
    out.push(`${scans} ${scans === 1 ? "page has" : "pages have"} no text and ${scans === 1 ? "is" : "are"} paired by position`);
  }
  out.push("pages matched by their text");
  return out;
}
