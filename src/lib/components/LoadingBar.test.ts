import { render, screen } from "@testing-library/svelte";
import { describe, expect, it } from "vitest";
import LoadingBar from "./LoadingBar.svelte";

describe("LoadingBar", () => {
  it("names the bar after its label and shows the full label on hover", () => {
    render(LoadingBar, { label: "Waiting for SM_CemeteryTerrain.fbx and 120 more…", value: 0.5 });
    const bar = screen.getByRole("progressbar", { name: "Waiting for SM_CemeteryTerrain.fbx and 120 more…" });
    expect(bar).toHaveAttribute("aria-valuenow", "50");
    expect(screen.getByTitle("Waiting for SM_CemeteryTerrain.fbx and 120 more…")).toBeInTheDocument();
    expect(bar).not.toHaveClass("wide");
  });

  it("keeps a fixed width when asked, for labels that change while it shows", () => {
    render(LoadingBar, { label: "Loading textures 3 / 132", wide: true });
    expect(screen.getByRole("progressbar")).toHaveClass("wide");
  });
});
