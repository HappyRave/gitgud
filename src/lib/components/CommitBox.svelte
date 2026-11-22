<script lang="ts">
  let {
    staged,
    branch = "",
    busy = false,
    oncommit,
  }: {
    /** Number of staged entries. */
    staged: number;
    /** Where the commit lands, for the hint line. */
    branch?: string;
    busy?: boolean;
    /** Resolves when the commit landed; rejects to keep the message. */
    oncommit: (message: string) => Promise<void>;
  } = $props();

  let message = $state("");
  let box = $state<HTMLTextAreaElement | null>(null);

  /** Puts the cursor in the message (C in the working copy). */
  export function focus() {
    box?.focus();
  }
  const ready = $derived(staged > 0 && message.trim() !== "" && !busy);
  const hint = $derived(
    staged === 0
      ? "Stage files to commit them"
      : `${staged} file${staged === 1 ? "" : "s"} staged${branch ? ` · on ${branch}` : ""}`,
  );

  async function submit() {
    if (!ready) {
      return;
    }
    try {
      await oncommit(message);
      message = "";
    } catch {
      // The caller shows the error; the message stays for another try.
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
  <textarea bind:this={box} bind:value={message} onkeydown={onKey} placeholder="Summary · what changed and why" aria-label="Commit message" rows="3"
  ></textarea>
  <div class="bt">
    <span class="ab">{hint}</span>
    <span class="sp"></span>
    <button type="submit" class="pri" disabled={!ready} title="Ctrl+Enter">
      {staged > 0 ? `Commit ${staged} file${staged === 1 ? "" : "s"}` : "Commit"}
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
