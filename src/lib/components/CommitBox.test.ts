import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import type { CommitOptions } from "$lib/api";
import { mockCommands } from "$lib/test/ipc";
import CommitBox from "./CommitBox.svelte";

const box = () => screen.getByRole("textbox", { name: "Commit message" });
const button = () => screen.getByRole("button", { name: /^(?:Commit(?: \d+ files?)?|Amend(?: .+)?)$/ });

function setup(props: {
  repoId?: number;
  staged: number;
  busy?: boolean;
  oncommit?: (m: string, options?: CommitOptions) => Promise<void>;
}) {
  const oncommit = vi.fn(props.oncommit ?? (async () => {}));
  render(CommitBox, { ...props, oncommit });
  return oncommit;
}

describe("CommitBox", () => {
  it("names how many files will be committed", () => {
    setup({ staged: 2 });
    expect(button()).toHaveTextContent("Commit 2 files");
  });

  it("is disabled until something is staged and a message is typed", async () => {
    setup({ staged: 0 });
    expect(button()).toBeDisabled();
    await fireEvent.input(box(), { target: { value: "msg" } });
    expect(button()).toBeDisabled();
  });

  it("ignores a whitespace-only message", async () => {
    setup({ staged: 1 });
    await fireEvent.input(box(), { target: { value: "  \n " } });
    expect(button()).toBeDisabled();
  });

  it("commits the message as typed and clears it", async () => {
    const oncommit = setup({ staged: 1 });
    await fireEvent.input(box(), { target: { value: "Subject\n\nBody" } });

    await fireEvent.click(button());

    expect(oncommit).toHaveBeenCalledWith("Subject\n\nBody");
    expect(box()).toHaveValue("");
  });

  it("commits on Ctrl+Enter", async () => {
    const oncommit = setup({ staged: 1 });
    await fireEvent.input(box(), { target: { value: "quick" } });

    await fireEvent.keyDown(box(), { key: "Enter", ctrlKey: true });

    expect(oncommit).toHaveBeenCalledWith("quick");
  });

  it("keeps the message when the commit fails", async () => {
    setup({
      staged: 1,
      oncommit: async () => {
        throw new Error("hook rejected");
      },
    });
    await fireEvent.input(box(), { target: { value: "keep me" } });

    await fireEvent.click(button());

    expect(box()).toHaveValue("keep me");
  });

  it("does nothing while busy", async () => {
    const oncommit = setup({ staged: 1, busy: true });
    await fireEvent.input(box(), { target: { value: "wait" } });

    await fireEvent.keyDown(box(), { key: "Enter", ctrlKey: true });

    expect(button()).toBeDisabled();
    expect(oncommit).not.toHaveBeenCalled();
  });

  it("amends HEAD with zero staged files, prefilling last commit message and resetting after commit", async () => {
    const calls = mockCommands({ head_commit_message: () => "previous subject" });
    const oncommit = setup({ repoId: 4, staged: 0 });

    await fireEvent.click(screen.getByRole("button", { name: "Commit options" }));
    await fireEvent.click(screen.getByLabelText("Amend"));

    await vi.waitFor(() => expect(box()).toHaveValue("previous subject"));
    expect(calls).toEqual([{ cmd: "head_commit_message", args: { id: 4 } }]);
    expect(button()).toHaveTextContent("Amend last commit");
    expect(button()).toBeEnabled();

    await fireEvent.click(button());

    expect(oncommit).toHaveBeenCalledWith("previous subject", {
      amend: true,
      signoff: false,
      noVerify: false,
    });
    expect(box()).toHaveValue("");
    expect(button()).toHaveTextContent("Commit");
  });

  it("passes signoff and noVerify flags when toggled", async () => {
    const oncommit = setup({ staged: 1 });
    await fireEvent.input(box(), { target: { value: "signed msg" } });

    await fireEvent.click(screen.getByRole("button", { name: "Commit options" }));
    await fireEvent.click(screen.getByLabelText(/Sign-off/));
    await fireEvent.click(screen.getByLabelText(/Skip hooks/));
    expect(screen.getByText("sign-off")).toBeInTheDocument();
    expect(screen.getByText("no-verify")).toBeInTheDocument();

    await fireEvent.click(button());

    expect(oncommit).toHaveBeenCalledWith("signed msg", {
      amend: false,
      signoff: true,
      noVerify: true,
    });
  });
});
