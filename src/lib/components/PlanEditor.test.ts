import { fireEvent, render, screen, waitFor, within } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import type { Commands, HistoryPlan, PlanCommit, PlanPrediction, PlanStep } from "$lib/api";
import { mockCommands } from "$lib/test/ipc";
import PlanEditor from "./PlanEditor.svelte";

const commit = (id: string, subject: string, action: PlanCommit["action"] = "pick"): PlanCommit => ({
  id: id.padEnd(40, "0"),
  shortId: id,
  subject,
  message: subject,
  author: "Max",
  time: 0,
  action,
});

const PLAN: HistoryPlan = {
  branch: "feature/stack",
  base: "6acf5f7".padEnd(40, "0"),
  baseShort: "6acf5f7",
  baseSubject: "Remove EmojiOne assets",
  commits: [commit("3493748", "Move blimp camera rgiht"), commit("6bb2f7d", "fixup! WIP engine", "fixup"), commit("4592a21", "WIP engine")],
  dependents: ["feature/next"],
  pushed: "origin/feature/stack",
};

const clean = (list: PlanStep[]): PlanPrediction => ({
  steps: list.map((s, i) => ({ id: s.id, step: list.length - i, outcome: s.action === "drop" ? { kind: "dropped" } : { kind: "clean" } })),
  before: list.length,
  after: list.filter((s) => !["squash", "fixup", "drop"].includes(s.action)).length,
  changed: 0,
});

/** Engine conflicts at step 1 when moved onto `master`. */
const conflicted = (list: PlanStep[]): PlanPrediction => ({
  steps: list.map((s) =>
    s.id === PLAN.commits[2].id
      ? { id: s.id, step: 1, outcome: { kind: "conflict", paths: ["Assets/Engine.cs"], tree: "t1" } }
      : { id: s.id, step: 2, outcome: { kind: "afterConflict", step: 1 } },
  ),
  before: 3,
  after: 2,
  changed: null,
});

function setup(predict: (args: Commands["plan_preview"]["args"]) => PlanPrediction = (a) => clean(a.steps), props = {}) {
  const calls = mockCommands({
    plan_preview: (args) => ({ prediction: predict(args), rows: 9 }),
    commit_detail: ({ commit: id }) => ({
      id,
      shortId: id.slice(0, 7),
      subject: "",
      body: "",
      author: { name: "Max", email: "m", time: 0 },
      committer: { name: "Max", email: "m", time: 0 },
      parents: [],
      files: [{ path: "Assets/Prefabs/ReplayCameras.prefab", change: "modified" }],
    }),
    commit_file_diff: ({ path }) => ({ path, binary: false, hunks: [] }),
    predicted_file: () => "a\n<<<<<<< ours\nmine\n=======\ntheirs\n>>>>>>> theirs\nb\n",
  });
  const onpreview = vi.fn();
  const onstart = vi.fn();
  const oncancel = vi.fn();
  render(PlanEditor, { repoId: 1, plan: PLAN, branches: ["master"], onpreview, onstart, oncancel, ...props });
  const lastSteps = () => (calls.filter((c) => c.cmd === "plan_preview").at(-1)?.args as Commands["plan_preview"]["args"]).steps;
  return { calls, onpreview, onstart, oncancel, lastSteps };
}

const list = () => screen.getByRole("listbox", { name: "Plan" });
const rows = () => within(list()).getAllByRole("option");
const press = (key: string, init: KeyboardEventInit = {}) => fireEvent.keyDown(list(), { key, ...init });

describe("PlanEditor", () => {
  it("lists the commits newest first with the fixup applied, and predicts them", async () => {
    const { onpreview } = setup();

    expect(rows().map((r) => r.textContent)).toEqual([
      expect.stringContaining("Move blimp camera rgiht"),
      expect.stringContaining('auto from "fixup!": folds into 4592a21'),
      expect.stringContaining("WIP engine"),
    ]);
    expect(await screen.findByText("✓ final files identical to now")).toBeInTheDocument();
    expect(screen.getByText("Result: 3 commits → 2")).toBeInTheDocument();
    expect(screen.getAllByText("✓ clean")).toHaveLength(3);
    expect(onpreview).toHaveBeenCalledWith(9);
    expect(screen.getByText(/feature\/stack is on origin\/feature\/stack/)).toBeInTheDocument();
  });

  it("R rewords the selected row with an inline message; the plan sends it", async () => {
    const { lastSteps, onstart } = setup();
    press("r");

    const box = await screen.findByRole("textbox", { name: "Message of 3493748" });
    await fireEvent.input(box, { target: { value: "Move blimp camera right" } });
    await waitFor(() => expect(lastSteps()[0]).toEqual({ id: PLAN.commits[0].id, action: "reword", message: "Move blimp camera right" }));

    await fireEvent.keyDown(box, { key: "Enter", ctrlKey: true });
    expect(onstart).toHaveBeenCalledWith(expect.objectContaining({ onto: PLAN.base, updateRefs: false }));
    expect(onstart.mock.calls[0][0].rows[0].message).toBe("Move blimp camera right");
  });

  it("S on the fixup opens the combined message; D drops; Alt+↑ moves", async () => {
    const { lastSteps } = setup();
    press("ArrowDown");
    press("s");
    expect(await screen.findByRole("textbox", { name: "Message of 6bb2f7d" })).toHaveValue("WIP engine\n\nfixup! WIP engine");

    await fireEvent.keyDown(screen.getByRole("textbox", { name: "Message of 6bb2f7d" }), { key: "Escape" });
    press("ArrowUp", { altKey: true });
    press("ArrowDown");
    press("d");

    await waitFor(() => expect(lastSteps().map((s) => [s.id.slice(0, 7), s.action])).toEqual([
      ["6bb2f7d", "squash"],
      ["3493748", "drop"],
      ["4592a21", "pick"],
    ]));
  });

  it("refuses a fold at the bottom and says why", async () => {
    setup();
    press("ArrowDown");
    press("ArrowDown");
    press("f");

    expect(await screen.findByRole("status")).toHaveTextContent("nothing older to fold into");
    expect(rows()[2]).toHaveTextContent("Pick");
  });

  it("a predicted conflict offers Show the conflict, Keep the old base and Start anyway", async () => {
    const { calls, onstart } = setup((a) => (a.onto === "master" ? conflicted(a.steps) : clean(a.steps)));
    await fireEvent.change(screen.getByRole("combobox", { name: "Onto" }), { target: { value: "master" } });

    expect(await screen.findByText(/1 conflict predicted at step 1/)).toHaveTextContent("WIP engine · Assets/Engine.cs");
    expect(rows()[2]).toHaveTextContent("step 1: Assets/Engine.cs would conflict with master");
    expect(rows()[0]).toHaveTextContent("✓ after step 1");
    press("Enter", { ctrlKey: true });
    expect(onstart).not.toHaveBeenCalled();

    await fireEvent.click(screen.getByRole("button", { name: "Show the conflict" }));
    const shown = await screen.findByLabelText("Predicted conflict in Assets/Engine.cs");
    expect(shown.querySelector(".yours")).toHaveTextContent("mine");
    expect(shown.querySelector(".theirs")).toHaveTextContent("theirs");
    expect(calls.find((c) => c.cmd === "predicted_file")?.args).toEqual({ id: 1, tree: "t1", path: "Assets/Engine.cs" });

    await fireEvent.click(screen.getByRole("button", { name: "Keep the old base" }));
    expect(await screen.findByText("✓ no conflicts predicted")).toBeInTheDocument();

    await fireEvent.change(screen.getByRole("combobox", { name: "Onto" }), { target: { value: "master" } });
    await fireEvent.click(await screen.findByRole("button", { name: "Start anyway" }));
    expect(onstart).toHaveBeenCalledWith(expect.objectContaining({ onto: "master" }));
  });

  it("Update dependent branches is passed to Start; Esc cancels", async () => {
    const { onstart, oncancel } = setup();
    await fireEvent.click(screen.getByRole("checkbox", { name: /Update dependent branches \(1\)/ }));
    await fireEvent.click(screen.getByRole("button", { name: /Start rebase/ }));
    expect(onstart).toHaveBeenCalledWith(expect.objectContaining({ updateRefs: true }));

    press("Escape");
    expect(oncancel).toHaveBeenCalled();
  });

  it("Ctrl+Enter starts from anywhere in the plan, e.g. the checkbox; the Onto picker keeps its arrows", async () => {
    const { onstart } = setup();
    await screen.findByText("✓ no conflicts predicted");
    const picker = screen.getByRole("combobox", { name: "Onto" });
    await fireEvent.keyDown(picker, { key: "ArrowDown" });
    expect(rows()[0]).toHaveAttribute("aria-selected", "true");

    await fireEvent.keyDown(screen.getByRole("checkbox", { name: /Update dependent branches/ }), { key: "Enter", ctrlKey: true });
    expect(onstart).toHaveBeenCalledTimes(1);
  });

  it("opens with a preset row, as R on an older commit does", async () => {
    setup(undefined, { preset: { id: PLAN.commits[2].id, action: "reword" } });
    expect(await screen.findByRole("textbox", { name: "Message of 4592a21" })).toHaveValue("WIP engine");
    expect(rows()[2]).toHaveAttribute("aria-selected", "true");
  });

  it("shows the selected commit's files below the list", async () => {
    setup();
    expect(await screen.findByText("· Assets/Prefabs/ReplayCameras.prefab")).toBeInTheDocument();
  });
});
