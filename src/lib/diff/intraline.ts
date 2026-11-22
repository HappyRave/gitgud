import type { Hunk } from "$lib/api";

export interface InlineSpan {
  text: string;
  changed: boolean;
}

const MAX_INLINE_LEN = 500;
const MAX_PAIR_LINES = 16;

function tokenize(text: string): string[] {
  return text.match(/[A-Za-z0-9_]+|\s+|./g) ?? [];
}

function pushSpan(out: InlineSpan[], text: string, changed: boolean) {
  if (!text) {
    return;
  }
  const prev = out[out.length - 1];
  if (prev && prev.changed === changed) {
    prev.text += text;
  } else {
    out.push({ text, changed });
  }
}

function lcsSpans(oldTokens: string[], newTokens: string[]): { left: InlineSpan[]; right: InlineSpan[] } {
  const m = oldTokens.length;
  const n = newTokens.length;
  const dp: number[][] = Array.from({ length: m + 1 }, () => new Array<number>(n + 1).fill(0));
  for (let i = m - 1; i >= 0; i--) {
    for (let j = n - 1; j >= 0; j--) {
      dp[i][j] = oldTokens[i] === newTokens[j] ? 1 + dp[i + 1][j + 1] : Math.max(dp[i + 1][j], dp[i][j + 1]);
    }
  }
  const left: InlineSpan[] = [];
  const right: InlineSpan[] = [];
  let i = 0;
  let j = 0;
  while (i < m && j < n) {
    if (oldTokens[i] === newTokens[j]) {
      pushSpan(left, oldTokens[i], false);
      pushSpan(right, newTokens[j], false);
      i++;
      j++;
    } else if (dp[i + 1][j] >= dp[i][j + 1]) {
      pushSpan(left, oldTokens[i], true);
      i++;
    } else {
      pushSpan(right, newTokens[j], true);
      j++;
    }
  }
  while (i < m) {
    pushSpan(left, oldTokens[i++], true);
  }
  while (j < n) {
    pushSpan(right, newTokens[j++], true);
  }
  return { left, right };
}

/** Computes word/token-level intra-line change spans for a paired old/new line. */
export function intralineDiff(oldLine: string, newLine: string): { left: InlineSpan[]; right: InlineSpan[] } {
  if (oldLine === newLine || oldLine.length > MAX_INLINE_LEN || newLine.length > MAX_INLINE_LEN) {
    return {
      left: oldLine ? [{ text: oldLine, changed: false }] : [],
      right: newLine ? [{ text: newLine, changed: false }] : [],
    };
  }

  const a = tokenize(oldLine);
  const b = tokenize(newLine);
  let prefix = 0;
  while (prefix < a.length && prefix < b.length && a[prefix] === b[prefix]) {
    prefix++;
  }
  let suffix = 0;
  while (
    suffix < a.length - prefix &&
    suffix < b.length - prefix &&
    a[a.length - 1 - suffix] === b[b.length - 1 - suffix]
  ) {
    suffix++;
  }

  const left: InlineSpan[] = [];
  const right: InlineSpan[] = [];
  const head = a.slice(0, prefix).join("");
  if (head) {
    pushSpan(left, head, false);
    pushSpan(right, head, false);
  }

  const midA = a.slice(prefix, a.length - suffix);
  const midB = b.slice(prefix, b.length - suffix);
  if (midA.length * midB.length <= 2500) {
    const mid = lcsSpans(midA, midB);
    for (const s of mid.left) {
      pushSpan(left, s.text, s.changed);
    }
    for (const s of mid.right) {
      pushSpan(right, s.text, s.changed);
    }
  } else {
    pushSpan(left, midA.join(""), true);
    pushSpan(right, midB.join(""), true);
  }

  const tail = suffix > 0 ? a.slice(a.length - suffix).join("") : "";
  if (tail) {
    pushSpan(left, tail, false);
    pushSpan(right, tail, false);
  }

  return { left, right };
}

/** Maps line index in `hunk.lines` to its intra-line spans when removed/added lines pair up cleanly. */
export function pairHunkLines(hunk: Hunk): Map<number, InlineSpan[]> {
  const map = new Map<number, InlineSpan[]>();
  let removedIdx: number[] = [];
  let addedIdx: number[] = [];

  const flush = () => {
    if (removedIdx.length > 0 && removedIdx.length === addedIdx.length && removedIdx.length <= MAX_PAIR_LINES) {
      for (let i = 0; i < removedIdx.length; i++) {
        const li = removedIdx[i];
        const ri = addedIdx[i];
        const { left, right } = intralineDiff(hunk.lines[li].text, hunk.lines[ri].text);
        map.set(li, left);
        map.set(ri, right);
      }
    }
    removedIdx = [];
    addedIdx = [];
  };

  for (let i = 0; i < hunk.lines.length; i++) {
    const kind = hunk.lines[i].kind;
    if (kind === "removed") {
      if (addedIdx.length > 0) {
        flush();
      }
      removedIdx.push(i);
    } else if (kind === "added") {
      addedIdx.push(i);
    } else {
      flush();
    }
  }
  flush();
  return map;
}
