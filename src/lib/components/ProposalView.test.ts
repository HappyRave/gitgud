import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import type { FileDiff, ProposalSummary } from "$lib/api";
import { mockCommands } from "$lib/test/ipc";
import ProposalView from "./ProposalView.svelte";

const PROPOSAL: ProposalSummary = {
  id: "p-1",
  commitId: "a".repeat(40),
  shortCommit: "aaaaaaa",
  baseId: "b".repeat(40),
  message: "Tune boat steering damping",
  why: "Reduces oscillation at high speed",
  agent: "claude-code",
  status: "pending",
  createdAt: 1_700_000_000,
  updatedAt: 1_700_000_000,
  files: ["Assets/Scripts/Engine.cs", "Assets/Scripts/Steering.cs"],
  riskFlags: [
    {
      kind: "missingMeta",
      path: "Assets/Scripts/Engine.cs",
      detail: "Missing companion .meta file",
    },
  ],
  comments: [],
  headMoved: true,
};

const diffOf = (path: string): FileDiff => ({
  path,
  binary: false,
  hunks: [
    {
      oldStart: 1,
      oldLines: 1,
      newStart: 1,
      newLines: 1,
      lines: [{ kind: "added", old: null, new: 1, text: `change in ${path}` }],
    },
  ],
});

function setup(proposal: ProposalSummary = PROPOSAL) {
  const calls = mockCommands({
    proposal_file_diff: ({ path }) => diffOf(path),
  });
  const onapprove = vi.fn();
  const onrequestchanges = vi.fn();
  const onreject = vi.fn();
  const onclose = vi.fn();
  render(ProposalView, {
    repoId: 3,
    proposal,
    onapprove,
    onrequestchanges,
    onreject,
    onclose,
  });
  return { calls, onapprove, onrequestchanges, onreject, onclose };
}

describe("ProposalView", () => {
  it("loads the first file diff, shows rationale and risk flags, and switches files", async () => {
    const { calls } = setup();

    expect(screen.getByText(/Reduces oscillation at high speed/)).toBeInTheDocument();
    expect(screen.getByText(/Missing companion \.meta file/)).toBeInTheDocument();
    expect(screen.getByText("HEAD moved — will replay")).toBeInTheDocument();
    expect(await screen.findByRole("region", { name: "Diff of Assets/Scripts/Engine.cs" })).toHaveTextContent(
      "change in Assets/Scripts/Engine.cs",
    );
    expect(calls).toContainEqual({
      cmd: "proposal_file_diff",
      args: { id: 3, proposalId: "p-1", path: "Assets/Scripts/Engine.cs" },
    });

    await fireEvent.click(screen.getByRole("button", { name: /Steering\.cs/ }));
    expect(await screen.findByRole("region", { name: "Diff of Assets/Scripts/Steering.cs" })).toHaveTextContent(
      "change in Assets/Scripts/Steering.cs",
    );
  });

  it("submits edited message and selected file paths on approve", async () => {
    const { onapprove } = setup();

    const msg = screen.getByLabelText("Proposal commit message") as HTMLInputElement;
    await fireEvent.input(msg, { target: { value: "Improve steering stability" } });

    const firstCheckbox = screen.getByLabelText("Include Assets/Scripts/Engine.cs");
    await fireEvent.click(firstCheckbox);

    await fireEvent.click(screen.getByRole("button", { name: "Approve & commit" }));
    expect(onapprove).toHaveBeenCalledWith("Improve steering stability", ["Assets/Scripts/Steering.cs"]);
  });

  it("submits request-changes feedback and supports reject", async () => {
    const { onrequestchanges, onreject } = setup();

    const input = screen.getByLabelText("Change request feedback") as HTMLInputElement;
    await fireEvent.input(input, { target: { value: "Include Engine.cs.meta too" } });
    await fireEvent.click(screen.getByRole("button", { name: "Request changes" }));

    expect(onrequestchanges).toHaveBeenCalledWith("Include Engine.cs.meta too");
    expect(input.value).toBe("");

    await fireEvent.click(screen.getByRole("button", { name: "Reject" }));
    expect(onreject).toHaveBeenCalledTimes(1);
  });
});
