<script lang="ts">
  import { onMount } from "svelte";
  import Dialog from "./Dialog.svelte";

  let {
    ahead,
    behind,
    upstream,
    busy = false,
    onpull,
    oncancel,
  }: {
    ahead: number;
    behind: number;
    upstream: string;
    busy?: boolean;
    onpull: (strategy: "merge" | "rebase", remember: boolean) => void;
    oncancel: () => void;
  } = $props();

  let strategy = $state<"merge" | "rebase">("rebase");
  let remember = $state(true);
  let goButton = $state<HTMLButtonElement | null>(null);

  onMount(() => {
    goButton?.focus();
  });
</script>

<Dialog title="Branch diverged from {upstream}" width={480} {oncancel}>
  <p class="desc">
    You have <b>{ahead}</b> local {ahead === 1 ? "commit" : "commits"} and <b>{upstream}</b> has
    <b>{behind}</b> {behind === 1 ? "commit" : "commits"}. Choose how to combine them:
  </p>
  <label class="opt" class:sel={strategy === "rebase"}>
    <input type="radio" name="pull-strategy" value="rebase" bind:group={strategy} />
    <div>
      <b>Rebase local commits on top</b>
      <span class="ab">Replays your {ahead} {ahead === 1 ? "commit" : "commits"} cleanly above {upstream}; no merge commit.</span>
    </div>
  </label>
  <label class="opt" class:sel={strategy === "merge"}>
    <input type="radio" name="pull-strategy" value="merge" bind:group={strategy} />
    <div>
      <b>Merge {upstream} into current branch</b>
      <span class="ab">Creates a merge commit joining both histories as they happened.</span>
    </div>
  </label>
  <label class="chk">
    <input type="checkbox" bind:checked={remember} />
    <span>Remember for this repository (<code>pull.rebase = {strategy === "rebase" ? "true" : "false"}</code>)</span>
  </label>
  {#snippet footer()}
    <span class="ab">Undoable with Ctrl+Z</span>
    <span class="sp"></span>
    <button onclick={oncancel}>Cancel <span class="k">Esc</span></button>
    <button class="pri" bind:this={goButton} disabled={busy} onclick={() => onpull(strategy, remember)}>
      Pull ({strategy}) <span class="k">↵</span>
    </button>
  {/snippet}
</Dialog>

<style>
  .desc {
    margin: 0;
  }
  .opt {
    display: flex;
    gap: 8px;
    align-items: flex-start;
    padding: 8px 10px;
    border: 1px solid var(--line);
    border-radius: 7px;
    background: var(--bg);
  }
  .opt.sel {
    border-color: var(--l0);
    background: var(--sel);
  }
  .opt div {
    display: grid;
    gap: 2px;
  }
  .chk {
    display: flex;
    gap: 6px;
    align-items: center;
    font-size: 12px;
  }
  .sp {
    flex: 1;
  }
</style>
