<script lang="ts">
  import { headCommitMessage, type CommitOptions } from "$lib/api";

  let {
    repoId,
    staged,
    branch = "",
    busy = false,
    oncommit,
    ondraft,
  }: {
    repoId?: number;
    /** Number of staged entries. */
    staged: number;
    /** Where the commit lands, for the hint line. */
    branch?: string;
    busy?: boolean;
    /** Resolves when the commit landed; rejects to keep the message. */
    oncommit: (message: string, options?: CommitOptions) => Promise<void>;
    /** Drafts a commit message from the staged diff. */
    ondraft?: () => Promise<string | null>;
  } = $props();

  let message = $state("");
  let amend = $state(false);
  let signoff = $state(false);
  let noVerify = $state(false);
  let showOptions = $state(false);
  let drafting = $state(false);
  let box = $state<HTMLTextAreaElement | null>(null);

  /** Puts the cursor in the message (C in the working copy). */
  export function focus() {
    box?.focus();
  }
  const ready = $derived((staged > 0 || amend) && message.trim() !== "" && !busy);
  const hint = $derived(
    amend
      ? `Amending last commit${staged > 0 ? ` + ${staged} file${staged === 1 ? "" : "s"}` : ""}`
      : staged === 0
        ? "Stage files to commit them"
        : `${staged} file${staged === 1 ? "" : "s"} staged${branch ? ` · on ${branch}` : ""}`,
  );
  const buttonLabel = $derived(
    amend
      ? staged > 0
        ? `Amend + ${staged} file${staged === 1 ? "" : "s"}`
        : "Amend last commit"
      : staged > 0
        ? `Commit ${staged} file${staged === 1 ? "" : "s"}`
        : "Commit",
  );

  async function toggleAmend(checked: boolean) {
    amend = checked;
    if (checked && repoId !== undefined && message.trim() === "") {
      try {
        const last = await headCommitMessage(repoId);
        if (last && message.trim() === "") {
          message = last;
        }
      } catch {
        // Unborn or unreadable HEAD; let the user type a message.
      }
    }
  }

  async function submit() {
    if (!ready) {
      return;
    }
    const hasOptions = amend || signoff || noVerify;
    try {
      if (hasOptions) {
        await oncommit(message, { amend, signoff, noVerify });
      } else {
        await oncommit(message);
      }
      message = "";
      amend = false;
      signoff = false;
      noVerify = false;
      showOptions = false;
    } catch {
      // The caller shows the error; the message stays for another try.
    }
  }

  async function runDraft() {
    if (!ondraft || busy || drafting || staged === 0) {
      return;
    }
    drafting = true;
    try {
      const drafted = await ondraft();
      if (drafted && drafted.trim() !== "") {
        message = drafted;
        box?.focus();
      }
    } finally {
      drafting = false;
    }
  }

  function onKey(event: KeyboardEvent) {
    if (event.key === "Enter" && event.ctrlKey) {
      event.preventDefault();
      submit();
    } else if (event.key === "Escape") {
      box?.blur();
    }
  }
</script>

<form
  aria-label="Commit"
  onsubmit={(e) => {
    e.preventDefault();
    submit();
  }}
>
  {#if showOptions}
    <div class="opts" role="group" aria-label="Commit options">
      <label>
        <input type="checkbox" checked={amend} onchange={(e) => toggleAmend((e.currentTarget as HTMLInputElement).checked)} />
        Amend
      </label>
      <label>
        <input type="checkbox" bind:checked={signoff} />
        Sign-off <code>-s</code>
      </label>
      <label>
        <input type="checkbox" bind:checked={noVerify} />
        Skip hooks <code>--no-verify</code>
      </label>
    </div>
  {/if}
  <textarea bind:this={box} bind:value={message} onkeydown={onKey} placeholder="Summary · what changed and why" aria-label="Commit message" rows="3"
  ></textarea>
  <div class="bt">
    <span class="ab">{hint}</span>
    {#if signoff}<span class="badge">sign-off</span>{/if}
    {#if noVerify}<span class="badge warn">no-verify</span>{/if}
    <span class="sp"></span>
    {#if ondraft}
      <button
        type="button"
        disabled={busy || drafting || staged === 0}
        title="Draft commit message from staged diff"
        onclick={runDraft}>{drafting ? "Drafting…" : "✦ Draft"}</button
      >
    {/if}
    <button
      type="button"
      class:on={showOptions || amend || signoff || noVerify}
      aria-label="Commit options"
      aria-expanded={showOptions}
      title="Amend, sign-off, skip hooks"
      onclick={() => (showOptions = !showOptions)}
    >⋯</button>
    <button type="submit" class="pri" disabled={!ready} title="Ctrl+Enter">
      {buttonLabel}
    </button>
  </div>
</form>

<style>
  form {
    display: grid;
    gap: 6px;
    padding: 8px 10px;
    border-top: 1px solid var(--line);
    background: var(--panel);
  }
  .opts {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    padding: 5px 8px;
    border-radius: 5px;
    background: var(--bg);
    border: 1px solid var(--line);
    font-size: 11.5px;
  }
  .opts label {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  textarea {
    resize: vertical;
    min-height: 52px;
  }
  .bt {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .sp {
    flex: 1;
  }
</style>
