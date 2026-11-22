<script lang="ts">
  import type { Conflict } from "$lib/api";

  let {
    conflicts,
    how,
    message = $bindable(),
    busy = false,
    oncommit,
  }: {
    conflicts: Conflict[];
    how: ReadonlyMap<string, string>;
    /** The merge commit message, prefilled by the page. */
    message: string;
    busy?: boolean;
    oncommit: () => void;
  } = $props();

  function onKeydown(event: KeyboardEvent) {
    if (event.key === "Enter" && event.ctrlKey) {
      event.preventDefault();
      oncommit();
    }
  }
</script>

<section class="summary" aria-label="Merge summary">
  <div class="ph">Ready to commit the merge</div>
  {#if conflicts.length}
    <div class="card">
      <div class="chd">
        <b>{conflicts.length} {conflicts.length === 1 ? "file" : "files"} resolved</b>
      </div>
      <ul>
        {#each conflicts as c (c.path)}
          <li><span class="ok">✓</span><span class="n">{c.path}</span><span class="p">{how.get(c.path) ?? "resolved"}</span></li>
        {/each}
      </ul>
    </div>
  {:else}
    <p class="ab pad">git merged every file on its own; nothing was left to resolve.</p>
  {/if}
  <div class="cbox">
    <textarea aria-label="Merge commit message" rows="6" bind:value={message} onkeydown={onKeydown}></textarea>
    <div class="bt">
      <span class="ab">Message prefilled; what was picked per file goes into the body.</span>
      <span class="sp"></span>
      <button class="pri" disabled={busy || !message.trim()} onclick={oncommit}>Commit merge <span class="k">Ctrl+↵</span></button>
    </div>
  </div>
</section>

<style>
  .summary {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
    overflow-y: auto;
    background: var(--panel);
  }
  .pad {
    padding: 10px;
    margin: 0;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 4px 0;
  }
  li {
    display: flex;
    gap: 8px;
    align-items: center;
    height: 24px;
    padding: 0 10px;
    white-space: nowrap;
  }
  .n {
    flex: none;
  }
  .p {
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .cbox {
    display: grid;
    gap: 6px;
    padding: 8px 10px;
  }
  textarea {
    resize: vertical;
    font: 12.5px/1.45 var(--mono);
  }
  .bt {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .sp {
    flex: 1;
  }
</style>
