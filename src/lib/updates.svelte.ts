import { type UpdateInfo, updateCheck } from "$lib/api";
import { prefs, setSkippedUpdate } from "$lib/prefs.svelte";

/** The update being offered in the popup, and the state of the last check. */
export const updates = $state({
  offer: null as UpdateInfo | null,
  checking: false,
  /** Outcome of the last manual check, for Settings: "up to date" or the error. */
  result: null as string | null,
});

/**
 * Looks for a newer release and offers it. The startup check (`manual` false) skips the
 * version the user skipped and only logs its errors; a manual one always offers and reports.
 */
export async function checkForUpdates(manual: boolean) {
  if (updates.checking) {
    return;
  }
  updates.checking = true;
  if (manual) {
    updates.result = null;
  }
  try {
    const found = await updateCheck(manual);
    if (!found) {
      if (manual) {
        updates.result = "gitgud is up to date.";
      }
      return;
    }
    if (manual || prefs.skippedUpdate !== found.version) {
      updates.offer = found;
    }
  } catch (e) {
    if (manual) {
      updates.result = String(e);
    } else {
      console.error("gitgud: update check failed:", e);
    }
  } finally {
    updates.checking = false;
  }
}

export function skipOffer() {
  if (updates.offer) {
    setSkippedUpdate(updates.offer.version);
  }
  updates.offer = null;
}

export function dismissOffer() {
  updates.offer = null;
}
