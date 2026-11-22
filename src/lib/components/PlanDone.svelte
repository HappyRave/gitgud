<script module lang="ts">
  /** What a finished history plan did, for the Done panel (R2 state 3). */
  export interface PlanResult {
    branch: string;
    /** Seconds since the Unix epoch. */
    time: number;
    before: number;
    after: number;
    /** Files the result differs in from before; null when unknown (a conflict was resolved). */
    changed: number | null;
    /** "Reworded 3493748 → "…" · folded 6bb2f7d into 4592a21". */
    summary: string;
    /** The upstream that still has the old commits. */
    pushed: string | null;
  }
</script>

<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    done,
    busy = false,
    onundo,
    onforcepush,
    onopenpr,
    children,
  }: {
    done: PlanResult;
    busy?: boolean;
    onundo: () => void;
    onforcepush?: () => void;
    onopenpr?: () => void;
    children?: Snippet;
  } = $props();

  const clock = $derived(new Date(done.time * 1000).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }));
</script>

<section class="done" aria-label="History edit done">
  <div class="ph">Done <span class="ct">· {clock}</span><span class="r">Esc close</span></div>
  <div class="card">
    <div class="chd"><b>Rebased {done.branch} · {done.before} commit{done.before === 1 ? "" : "s"} → {done.after}</b></div>
    <div class="cbd">
      {#if done.changed === 0}
        <span class="ok">✓ Final files identical to before</span>
      {:else if done.changed !== null}
        <span>Final files differ from before in {done.changed} file{done.changed === 1 ? "" : "s"}</span>
      {/if}
      {#if done.summary}<span>{done.summary}</span>{/if}
      {#if done.pushed}
        <div class="flag">{done.pushed} still has the old {done.before} commit{done.before === 1 ? "" : "s"}. Push replaces them.</div>
      {/if}
      <div class="btns">
        {#if done.pushed && onforcepush}
          <button class="pri" disabled={busy} onclick={onforcepush}>Force-push with lease…</button>
        {/if}
        {#if done.pushed && onopenpr}
          <button disabled={busy} onclick={onopenpr}>Open Pull Request ↗</button>
        {/if}
        <button disabled={busy} onclick={onundo}>Undo rebase <span class="k">Ctrl+Z</span></button>
      </div>
    </div>
  </div>
  {@render children?.()}
</section>

<style>
  .done {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-height: 0;
    overflow-y: auto;
  }
  .btns {
    display: flex;
    gap: 6px;
  }
</style>
