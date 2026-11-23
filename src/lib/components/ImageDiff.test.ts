import { fireEvent, render, screen } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import ImageDiff from "./ImageDiff.svelte";

const BEFORE = { label: "HEAD", url: "http://gitgud.localhost/image?at=head" };
const AFTER = { label: "On disk", url: "http://gitgud.localhost/image?at=worktree" };

function load(img: HTMLElement, w: number, h: number) {
  Object.defineProperty(img, "naturalWidth", { value: w });
  Object.defineProperty(img, "naturalHeight", { value: h });
  return fireEvent.load(img);
}

afterEach(() => vi.unstubAllGlobals());

describe("ImageDiff", () => {
  it("shows both versions side by side with their pixel size", async () => {
    render(ImageDiff, { before: BEFORE, after: AFTER });

    await load(screen.getByAltText("HEAD"), 2048, 2048);
    await load(screen.getByAltText("On disk"), 1024, 512);

    expect(screen.getByAltText("HEAD")).toHaveAttribute("src", BEFORE.url);
    expect(screen.getByText("2048 × 2048")).toBeInTheDocument();
    expect(screen.getByText("1024 × 512")).toBeInTheDocument();
  });

  it("says why a version is missing, from the protocol's answer", async () => {
    vi.stubGlobal("fetch", vi.fn(async () => new Response("t.png: this LFS version isn't downloaded", { status: 404 })));
    render(ImageDiff, { before: BEFORE, after: AFTER });

    await fireEvent.error(screen.getByAltText("HEAD"));

    expect(await screen.findByText("t.png: this LFS version isn't downloaded")).toBeInTheDocument();
    expect(screen.queryByAltText("HEAD")).toBeNull();
    expect(screen.getByRole("radio", { name: "Swipe" })).toBeDisabled();
  });

  it("swipe and onion overlay the two once both loaded", async () => {
    render(ImageDiff, { before: BEFORE, after: AFTER });
    expect(screen.getByRole("radio", { name: "Onion" })).toBeDisabled();
    await load(screen.getByAltText("HEAD"), 10, 10);
    await load(screen.getByAltText("On disk"), 10, 10);

    await fireEvent.click(screen.getByRole("radio", { name: "Swipe" }));
    expect(screen.getByAltText("On disk")).toHaveClass("swipe");
    await fireEvent.input(screen.getByRole("slider", { name: "Swipe position" }), { target: { value: "20" } });
    expect((screen.getByAltText("On disk").closest(".stack") as HTMLElement).style.getPropertyValue("--mix")).toBe("20%");

    await fireEvent.click(screen.getByRole("radio", { name: "Onion" }));
    expect(screen.getByAltText("On disk")).toHaveStyle({ opacity: "0.2" });
  });

  it("supports channel isolation, zoom, pixelated, backdrop, and Diff mode", async () => {
    render(ImageDiff, { before: BEFORE, after: AFTER, path: "Art/Hero_D.dds" });
    expect(screen.getByText("DDS")).toBeInTheDocument();

    await load(screen.getByAltText("HEAD"), 32, 32);
    await load(screen.getByAltText("On disk"), 32, 32);

    expect(screen.getByRole("radio", { name: "Diff" })).toBeEnabled();

    await fireEvent.click(screen.getByRole("radio", { name: "R" }));
    expect(screen.getByAltText("On disk").getAttribute("style")).toContain("-r");

    await fireEvent.click(screen.getByRole("button", { name: "Pixelated" }));
    expect(screen.getByAltText("On disk")).toHaveClass("pixelated");

    await fireEvent.click(screen.getByRole("radio", { name: "Dark" }));
    expect(screen.getByAltText("On disk").closest(".frame")).toHaveClass("bg-dark");

    await fireEvent.click(screen.getByRole("button", { name: "Toggle fit zoom" }));
    expect(screen.getByRole("button", { name: "Toggle fit zoom" })).toHaveTextContent("100%");
    await fireEvent.click(screen.getByRole("button", { name: "Zoom in" }));
    expect(screen.getByRole("button", { name: "Toggle fit zoom" })).toHaveTextContent("200%");

    await fireEvent.click(screen.getByRole("radio", { name: "Diff" }));
    expect(screen.getByRole("radiogroup", { name: "Diff gain" })).toBeInTheDocument();
  });

  it("gates unhydrated LFS images above 25 MiB behind an explicit Fetch button", async () => {
    const onfetch = vi.fn(async () => {});
    const largeMeta = { size: 30 * 1024 * 1024, lfs: true, oid: "ab".repeat(32), hydrated: false };
    render(ImageDiff, {
      before: BEFORE,
      after: AFTER,
      path: "Art/Big.exr",
      beforeMeta: { size: 1024, lfs: true, oid: "cd".repeat(32), hydrated: true },
      afterMeta: largeMeta,
      onfetch,
    });

    expect(screen.queryByAltText("On disk")).toBeNull();
    expect(screen.getByAltText("HEAD")).toBeInTheDocument();
    expect(screen.getByText("Pointer only")).toBeInTheDocument();
    expect(screen.getByText("Hydrated")).toBeInTheDocument();

    await fireEvent.click(screen.getByRole("button", { name: "Fetch LFS object (30 MB)" }));
    expect(onfetch).toHaveBeenCalledWith("after", largeMeta);
  });
});
