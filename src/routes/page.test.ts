import { render, screen } from "@testing-library/svelte";
import { describe, expect, it } from "vitest";
import { mockCommands } from "$lib/test/ipc";
import Page from "./+page.svelte";

describe("start page", () => {
  it("shows a loading line while git is being located", () => {
    mockCommands({ git_info: () => new Promise(() => {}) });
    render(Page);

    expect(screen.getByText("Looking for git…")).toBeInTheDocument();
  });

  it("shows the git version and path", async () => {
    mockCommands({ git_info: () => ({ path: "C:/git/git.exe", version: "2.56.1" }) });
    render(Page);

    const line = await screen.findByTestId("git-info");
    expect(line).toHaveTextContent("git 2.56.1 C:/git/git.exe");
  });

  it("shows the error when git is unusable", async () => {
    mockCommands({
      git_info: () => {
        throw "git 2.30.0 is too old; gitgud needs 2.45.2 or newer";
      },
    });
    render(Page);

    expect(await screen.findByRole("alert")).toHaveTextContent("git 2.30.0 is too old; gitgud needs 2.45.2 or newer");
  });
});
