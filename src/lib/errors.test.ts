import { describe, expect, it } from "vitest";
import { explainError } from "./errors";

describe("explainError", () => {
  it("explains common git errors with plain words and actionable fixes", () => {
    const pushRejected = explainError("! [rejected] main -> main (non-fast-forward)\nhint: Updates were rejected because the tip of your current branch is behind");
    expect(pushRejected.title).toContain("newer commits");
    expect(pushRejected.fix?.id).toBe("pull");

    const identity = explainError("fatal: unable to auto-detect email address\nAuthor identity unknown");
    expect(identity.title).toContain("identity");
    expect(identity.fix?.id).toBe("settings");

    const dirty = explainError("error: Your local changes to the following files would be overwritten by checkout:");
    expect(dirty.fix?.id).toBe("stash");

    const lfs = explainError("Error downloading object: Assets/Art/Tex.psd (LFS: smudge error)");
    expect(lfs.concept).toBe("Git LFS");
    expect(lfs.fix?.id).toBe("settings");
  });

  it("falls back cleanly while keeping raw message intact", () => {
    const unknown = explainError("something unexpected happened\nsecond line");
    expect(unknown.title).toBe("Action failed");
    expect(unknown.explanation).toBe("something unexpected happened");
    expect(unknown.raw).toContain("something unexpected happened");
  });
});
