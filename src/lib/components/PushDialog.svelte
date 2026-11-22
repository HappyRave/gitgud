<script lang="ts">
  import { onMount } from "svelte";
  import { openExternalUrl, pushInfo, type PushInfo } from "$lib/api";
  import Dialog from "./Dialog.svelte";

  let {
    repoId,
    initialForce = false,
    busy = false,
    onpush,
    oncancel,
  }: {
    repoId: number;
    /** Pre-enables `--force-with-lease` when opened from the Done panel. */
    initialForce?: boolean;
    busy?: boolean;
    onpush: (opts: { remote: string; branch: string; remoteBranch: string; setUpstream: boolean; forceWithLease: boolean }) => void;
    oncancel: () => void;
  } = $props();

  let info = $state<PushInfo | null>(null);
  let error = $state<string | null>(null);
  let remote = $state("");
  let remoteBranch = $state("");
  let setUpstream = $state(false);
  let forceWithLease = $state(false);
  let cancelButton = $state<HTMLButtonElement | null>(null);
  let pushButton = $state<HTMLButtonElement | null>(null);

  function load(targetRemote?: string) {
    error = null;
    pushInfo(repoId, targetRemote).then(
      (res) => {
        info = res;
        remote = res.remote ?? res.remotes[0] ?? "";
        remoteBranch = res.remoteBranch;
        setUpstream = !res.upstream;
        forceWithLease = initialForce || res.needsForce;
        if (forceWithLease) {
          cancelButton?.focus();
        } else {
          pushButton?.focus();
        }
      },
      (err) => (error = String(err)),
    );
  }

  onMount(() => {
    forceWithLease = initialForce;
    load();
  });

  function submit() {
    if (!info || !remote || busy) {
      return;
    }
    onpush({
      remote,
      branch: info.branch,
      remoteBranch: remoteBranch.trim() || info.branch,
      setUpstream,
      forceWithLease,
    });
  }
</script>

<Dialog title={info ? (info.upstream ? `Push ${info.branch}` : `Publish ${info.branch}`) : "Push"} width={500} {oncancel}>
  {#if error}
    <p role="alert" class="flag bad">{error}</p>
  {:else if !info}
    <p class="ab">Checking commits to push…</p>
  {:else if info.remotes.length === 0}
    <p class="flag bad">No remote configured in this repository.</p>
  {:else}
    <div class="row">
      <label>
        <span class="ab">Remote</span>
        <select
          aria-label="Remote"
          value={remote}
          onchange={(e) => load((e.currentTarget as HTMLSelectElement).value)}
        >
          {#each info.remotes as r (r)}
            <option value={r}>{r}</option>
          {/each}
        </select>
      </label>
      <label class="grow">
        <span class="ab">Target branch on {remote}</span>
        <input type="text" aria-label="Target branch" bind:value={remoteBranch} />
      </label>
    </div>

    {#if info.behind > 0}
      <div class="flag bad">
        <b>{info.upstream ?? `${remote}/${remoteBranch}`}</b> has <b>{info.behind}</b>
        {info.behind === 1 ? "commit" : "commits"} not on <b>{info.branch}</b>. Force-pushing with lease replaces them on the remote.
      </div>
    {/if}

    {#if info.ahead === 0 && !forceWithLease}
      <p class="ok">✓ Nothing new to push ({info.upstream ?? `${remote}/${remoteBranch}`} is up to date).</p>
    {:else if info.ahead > 0}
      <p class="ab">
        {info.ahead} {info.ahead === 1 ? "commit" : "commits"} to send to <b>{remote}/{remoteBranch}</b>:
      </p>
      <ul class="commits">
        {#each info.commits.slice(0, 8) as c (c.shortId)}
          <li><code>{c.shortId}</code> {c.subject}</li>
        {/each}
        {#if info.ahead > 8}
          <li class="ab">and {info.ahead - 8} more…</li>
        {/if}
      </ul>
    {/if}

    <div class="checks">
      {#if !info.upstream}
        <label class="chk">
          <input type="checkbox" bind:checked={setUpstream} />
          <span>Set upstream tracking (<code>--set-upstream</code>)</span>
        </label>
      {/if}
      <label class="chk">
        <input type="checkbox" bind:checked={forceWithLease} />
        <span>Force-push safely (<code>--force-with-lease</code>)</span>
      </label>
    </div>
  {/if}

  {#snippet footer()}
    {#if info?.prUrl}
      <button type="button" onclick={() => info?.prUrl && openExternalUrl(info.prUrl)}>Open Pull Request ↗</button>
    {:else}
      <span class="ab">Can't be undone remotely</span>
    {/if}
    <span class="sp"></span>
    <button bind:this={cancelButton} onclick={oncancel}>Cancel <span class="k">Esc</span></button>
    <button
      class="pri"
      class:bad={forceWithLease}
      bind:this={pushButton}
      disabled={!info || !remote || busy || (info.ahead === 0 && !forceWithLease)}
      onclick={submit}
    >
      {forceWithLease ? "Force-push with lease" : info?.upstream ? "Push" : "Publish branch"}
    </button>
  {/snippet}
</Dialog>

<style>
  .row {
    display: flex;
    gap: 8px;
  }
  label {
    display: grid;
    gap: 3px;
  }
  .grow {
    flex: 1;
  }
  select {
    font: inherit;
    color: var(--fg);
    background: var(--bg);
    border: 1px solid var(--line);
    border-radius: 5px;
    padding: 5px 8px;
  }
  p {
    margin: 0;
  }
  .commits {
    margin: 0;
    padding-left: 16px;
    font-size: 12px;
    max-height: 150px;
    overflow-y: auto;
  }
  .commits li {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .checks {
    display: grid;
    gap: 4px;
  }
  .chk {
    display: flex;
    gap: 6px;
    align-items: center;
  }
  .sp {
    flex: 1;
  }
  button.bad {
    background: var(--bad-ink);
    border-color: var(--bad-ink);
    color: #fff;
  }
</style>
