import { gitLatest, type LatestGit } from "$lib/api";

/** The newest Git for Windows, from the startup check or Settings › Git's button. */
export const gitRelease = $state({
  latest: null as LatestGit | null,
  checking: false,
  error: null as string | null,
});

/** The startup check (`manual` false) only logs its errors; a manual one reports them. */
export async function checkLatestGit(manual: boolean) {
  if (gitRelease.checking) {
    return;
  }
  gitRelease.checking = true;
  gitRelease.error = null;
  try {
    gitRelease.latest = await gitLatest();
  } catch (e) {
    if (manual) {
      gitRelease.error = String(e);
    } else {
      console.error("gitgud: Git for Windows version check failed:", e);
    }
  } finally {
    gitRelease.checking = false;
  }
}
