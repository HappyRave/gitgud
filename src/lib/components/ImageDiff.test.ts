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
});
