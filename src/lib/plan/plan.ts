import type { HistoryPlan, PlanAction, PlanCommit, PlanPrediction, PlanStep, StepPrediction } from "$lib/api";

/** One plan row as the user edits it; newest first, like the graph. */
export interface PlanRow {
  commit: PlanCommit;
  action: PlanAction;
  /** The new message of a reword, or the combined message of a squash. */
  message: string | null;
}

export const ACTIONS: { key: string; action: PlanAction; label: string }[] = [
  { key: "P", action: "pick", label: "Pick" },
  { key: "R", action: "reword", label: "Reword" },
  { key: "E", action: "edit", label: "Edit" },
  { key: "S", action: "squash", label: "Squash" },
  { key: "F", action: "fixup", label: "Fixup" },
  { key: "D", action: "drop", label: "Drop" },
];

export const folds = (a: PlanAction) => a === "squash" || a === "fixup";

/** The rows a plan opens with; `preset` sets one commit's action (R on an older commit). */
export function planRows(plan: HistoryPlan, preset?: { id: string; action: PlanAction }): PlanRow[] {
  let rows: PlanRow[] = plan.commits.map((commit) => ({ commit, action: commit.action, message: null }));
  const at = preset ? rows.findIndex((r) => r.commit.id === preset.id) : -1;
  if (preset && at >= 0) {
    rows = setAction(rows, at, preset.action) ?? rows;
  }
  return rows;
}

/** The row a squash or fixup at `index` folds into: the next older one not dropped. */
export function target(rows: PlanRow[], index: number): number | null {
  for (let i = index + 1; i < rows.length; i++) {
    if (rows[i].action !== "drop") {
      return i;
    }
  }
  return null;
}

/** The message row `index` ends up with, as the plan stands. */
export function effectiveMessage(rows: PlanRow[], index: number): string {
  const row = rows[index];
  return row.action === "reword" && row.message !== null ? row.message : row.commit.message;
}

/**
 * Rows with `action` on row `index`; `null` when it can't apply (a squash or
 * fixup with nothing older to fold into). Reword starts from the commit's
 * message; squash from both messages, the target's first.
 */
export function setAction(rows: PlanRow[], index: number, action: PlanAction): PlanRow[] | null {
  const next = rows.slice();
  const row = rows[index];
  let message: string | null = null;
  if (folds(action)) {
    const into = target(rows, index);
    if (into === null) {
      return null;
    }
    if (action === "squash") {
      message = row.action === "squash" && row.message !== null ? row.message : `${effectiveMessage(rows, into)}\n\n${row.commit.message}`;
    }
  } else if (action === "reword") {
    message = row.action === "reword" && row.message !== null ? row.message : row.commit.message;
  }
  next[index] = { ...row, action, message };
  return valid(next) ? next : null;
}

export function setMessage(rows: PlanRow[], index: number, message: string): PlanRow[] {
  const next = rows.slice();
  next[index] = { ...rows[index], message };
  return next;
}

/** Rows with row `from` moved to `to`; `null` when that leaves the oldest row folding into nothing. */
export function move(rows: PlanRow[], from: number, to: number): PlanRow[] | null {
  if (to < 0 || to >= rows.length || from === to) {
    return null;
  }
  const next = rows.slice();
  const [row] = next.splice(from, 1);
  next.splice(to, 0, row);
  return valid(next) ? next : null;
}

/** Whether git could run the plan: the oldest kept row doesn't fold. */
export function valid(rows: PlanRow[]): boolean {
  const oldest = [...rows].reverse().find((r) => r.action !== "drop");
  return !oldest || !folds(oldest.action);
}

/** Whether row `index` is the newest squash of its chain: the one whose message the plan uses. */
export function editsSquashMessage(rows: PlanRow[], index: number): boolean {
  if (rows[index].action !== "squash") {
    return false;
  }
  for (let i = index - 1; i >= 0 && rows[i].action !== "pick" && rows[i].action !== "reword" && rows[i].action !== "edit"; i--) {
    if (rows[i].action === "squash") {
      return false;
    }
  }
  return true;
}

export function steps(rows: PlanRow[]): PlanStep[] {
  return rows.map((r) => ({
    id: r.commit.id,
    action: r.action,
    message: r.action === "reword" || r.action === "squash" ? r.message : null,
  }));
}

/** The first step predicted to conflict, oldest first. */
export function firstConflict(prediction: PlanPrediction): StepPrediction | null {
  return [...prediction.steps].reverse().find((s) => s.outcome.kind === "conflict") ?? null;
}

/** One line on what happened, for the Done panel: `Reworded 3493748 → "…" · folded 6bb2f7d into 4592a21`. */
export function describe(rows: PlanRow[]): string {
  const parts: string[] = [];
  rows.forEach((r, i) => {
    const id = r.commit.shortId;
    if (r.action === "reword" && r.message !== null && r.message !== r.commit.message) {
      parts.push(`reworded ${id} → "${r.message.split("\n")[0]}"`);
    } else if (folds(r.action)) {
      const into = target(rows, i);
      parts.push(`${r.action === "squash" ? "squashed" : "folded"} ${id} into ${into === null ? "?" : rows[into].commit.shortId}`);
    } else if (r.action === "drop") {
      parts.push(`dropped ${id}`);
    } else if (r.action === "edit") {
      parts.push(`edited ${id}`);
    }
  });
  const text = parts.join(" · ");
  return text.charAt(0).toUpperCase() + text.slice(1);
}
