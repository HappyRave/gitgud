<script lang="ts">
  let {
    staged,
    busy = false,
    oncommit,
  }: {
    /** Number of staged entries. */
    staged: number;
    busy?: boolean;
    /** Resolves when the commit landed; rejects to keep the message. */
    oncommit: (message: string) => Promise<void>;
  } = $props();

  let message = $state("");
  const ready = $derived(staged > 0 && message.trim() !== "" && !busy);

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
  <textarea bind:value={message} onkeydown={onKey} placeholder="Commit message" aria-label="Commit message" rows="3"
  ></textarea>
  <button type="submit" disabled={!ready}>
    {staged > 0 ? `Commit ${staged} file${staged === 1 ? "" : "s"}` : "Commit"}
  </button>
</form>

<style>
  form {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    margin-top: 1rem;
  }
  textarea {
    font: inherit;
    font-size: 0.85rem;
    resize: vertical;
  }
  button {
    align-self: flex-end;
  }
</style>
