<script lang="ts">
  import { SvelteSet } from "svelte/reactivity";
  import { proposalFileDiff, type FileDiff, type ProposalStatus, type ProposalSummary } from "$lib/api";
  import { inTextField } from "$lib/keys";
  import { prefs } from "$lib/prefs.svelte";
  import DiffView from "./DiffView.svelte";

  let {
    repoId,
    proposal,
    busy = false,
    onapprove,
    onrequestchanges,
    onreject,
    onclose,
  }: {
    repoId: number;
    proposal: ProposalSummary;
    busy?: boolean;
    onapprove: (message: string, selectedPaths: string[]) => void;
    onrequestchanges: (comment: string) => void;
    onreject: () => void;
    onclose: () => void;
  } = $props();

  const STATUS_LABEL: Record<ProposalStatus, string> = {
    pending: "Pending",
    changesRequested: "Changes requested",
    approved: "Approved",
    rejected: "Rejected",
    withdrawn: "Withdrawn",
  };

  let lastId = $state<string | null>(null);
  let message = $state("");
  const checked = new SvelteSet<string>();
  let activeFile = $state<string | null>(null);
  let diff = $state<FileDiff | null>(null);
  let diffError = $state<string | null>(null);
  let feedback = $state("");
  let feedbackInput = $state<HTMLInputElement | null>(null);

  const chosenPaths = $derived(proposal.files.filter((f) => checked.has(f)));
  const canApprove = $derived(!busy && message.trim().length > 0 && chosenPaths.length > 0);

  $effect(() => {
    const p = proposal;
    if (p.id !== lastId) {
      lastId = p.id;
      message = p.message;
      checked.clear();
      for (const path of p.files) {
        checked.add(path);
      }
      activeFile = p.files[0] ?? null;
      feedback = "";
    }
  });

  $effect(() => {
    const pId = proposal.id;
    const path = activeFile;
    if (!path) {
      diff = null;
      diffError = null;
      return;
    }
    diff = null;
    diffError = null;
    let stale = false;
    proposalFileDiff(repoId, pId, path).then(
      (d) => {
        if (!stale && proposal.id === pId && activeFile === path) {
          diff = d;
        }
      },
      (err) => {
        if (!stale && proposal.id === pId && activeFile === path) {
          diffError = String(err);
        }
      },
    );
    return () => {
      stale = true;
    };
  });

  function toggleFile(path: string) {
    if (checked.has(path)) {
      checked.delete(path);
    } else {
      checked.add(path);
    }
  }

  function submitApprove() {
    if (!canApprove) {
      return;
    }
    onapprove(message.trim(), chosenPaths);
  }

  function submitFeedback() {
    const text = feedback.trim();
    if (busy || !text) {
      return;
    }
    feedback = "";
    onrequestchanges(text);
  }

  function onKey(event: KeyboardEvent) {
    if (event.ctrlKey && !event.altKey && !event.metaKey) {
      if (event.key === "Enter" && !event.shiftKey) {
        if (canApprove) {
          event.preventDefault();
          submitApprove();
        }
        return;
      }
      const lower = event.key.toLowerCase();
      if (event.shiftKey && lower === "r") {
        event.preventDefault();
        if (feedback.trim() && !busy) {
          submitFeedback();
        } else {
          feedbackInput?.focus();
        }
        return;
      }
      if (event.shiftKey && lower === "x") {
        if (!busy) {
          event.preventDefault();
          onreject();
        }
        return;
      }
    }
    if (event.ctrlKey || event.altKey || event.metaKey || inTextField(event.target) || proposal.files.length === 0) {
      return;
    }
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      const idx = activeFile ? proposal.files.indexOf(activeFile) : -1;
      const delta = event.key === "ArrowDown" ? 1 : -1;
      const next = idx === -1 ? 0 : Math.min(proposal.files.length - 1, Math.max(0, idx + delta));
      activeFile = proposal.files[next] ?? null;
      return;
    }
    if (event.key === " " && activeFile && !(event.target instanceof HTMLInputElement)) {
      event.preventDefault();
      toggleFile(activeFile);
    }
  }
</script>

<svelte:window onkeydown={onKey} />

<section class="detail" aria-label="Proposal review">
  <div class="ph">
    Proposal
    <span class="ct">· {proposal.agent} · {proposal.shortCommit}</span>
    <span class="pill" class:warn={proposal.status === "changesRequested"}>{STATUS_LABEL[proposal.status]}</span>
    {#if proposal.headMoved}
      <span class="pill warn" title="HEAD moved since this proposal was created; approving will 3-way replay it onto the current branch"
        >HEAD moved — will replay</span
      >
    {/if}
    <span class="r">
      <button type="button" class="close" onclick={onclose} aria-label="Back to working copy" title="Esc in the graph">✕ Close</button>
    </span>
  </div>

  <div class="card">
    <div class="cbd">
      <label class="msg-wrap">
        <span class="lbl">Commit message</span>
        <input class="msg" aria-label="Proposal commit message" bind:value={message} disabled={busy} />
      </label>
      {#if proposal.why.trim()}
        <p class="why"><span class="lbl">Why:</span> {proposal.why}</p>
      {/if}
      {#if proposal.riskFlags.length > 0}
        <ul class="risks" aria-label="Risk flags">
          {#each proposal.riskFlags as flag (flag.kind + ":" + flag.path + ":" + flag.detail)}
            <li>
              <span class="risk-ic" aria-hidden="true">⚠</span>
              <span class="risk-detail">{flag.detail}</span>
              {#if flag.path}
                <code class="risk-path">{flag.path}</code>
              {/if}
            </li>
          {/each}
        </ul>
      {/if}
      {#if proposal.comments.length > 0}
        <div class="comments" aria-label="Review comments">
          {#each proposal.comments as c (c.id)}
            <div class="comment">
              <span class="comment-author">{c.author}:</span>
              <span>{c.text}</span>
            </div>
          {/each}
        </div>
      {/if}
    </div>
  </div>

  <div class="grp">
    {chosenPaths.length} of {proposal.files.length}
    {proposal.files.length === 1 ? "file" : "files"} selected
  </div>
  <ul class="files" aria-label="Proposed files">
    {#each proposal.files as f (f)}
      {@const cut = f.lastIndexOf("/") + 1}
      <li>
        <div class="frow" class:current={f === activeFile}>
          <input type="checkbox" aria-label="Include {f}" checked={checked.has(f)} disabled={busy} onchange={() => toggleFile(f)} />
          <button
            type="button"
            class="bare fbtn"
            aria-pressed={f === activeFile}
            title={f}
            onclick={() => (activeFile = f)}
          >
            <span class="fp"><span class="dir">{f.slice(0, cut)}</span>{f.slice(cut)}</span>
          </button>
        </div>
      </li>
    {/each}
  </ul>

  <div class="diff-wrap">
    {#if diffError}
      <p role="alert" class="pad">{diffError}</p>
    {:else if diff}
      <DiffView {diff} split={prefs.split} />
    {:else if activeFile}
      <p class="note">Loading…</p>
    {:else}
      <p class="note">This proposal has no files.</p>
    {/if}
  </div>

  <div class="actions">
    <div class="feedback-row">
      <input
        bind:this={feedbackInput}
        class="feedback"
        aria-label="Change request feedback"
        placeholder="Request changes for {proposal.agent}… (Ctrl+Shift+R)"
        bind:value={feedback}
        disabled={busy}
        onkeydown={(e) => {
          if (e.key === "Enter" && !e.ctrlKey && !e.shiftKey && feedback.trim() && !busy) {
            e.preventDefault();
            submitFeedback();
          }
        }}
      />
      <button type="button" disabled={busy || !feedback.trim()} onclick={submitFeedback}>Request changes</button>
    </div>
    <div class="btn-row">
      <button type="button" class="danger" disabled={busy} title="Reject proposal (Ctrl+Shift+X)" onclick={onreject}>Reject</button>
      <button type="button" class="pri" disabled={!canApprove} title="Approve and commit selected files (Ctrl+Enter)" onclick={submitApprove}
        >Approve & commit</button
      >
    </div>
  </div>
</section>

<style>
  .detail {
    display: flex;
    flex-direction: column;
    min-height: 0;
    height: 100%;
  }
  .close {
    height: 20px;
    padding: 0 7px;
    font-size: 11px;
  }
  .pill {
    padding: 1px 6px;
    border-radius: 3px;
    font: 600 10.5px var(--mono);
    background: var(--hover);
    color: var(--muted);
  }
  .pill.warn {
    background: var(--warn);
    color: var(--warn-ink);
  }
  .cbd {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .msg-wrap {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .lbl {
    font: 600 10.5px var(--mono);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--muted);
  }
  .msg,
  .feedback {
    box-sizing: border-box;
    width: 100%;
    height: 26px;
    padding: 0 8px;
    border: 1px solid var(--line);
    border-radius: 4px;
    background: var(--panel);
    color: var(--fg);
    font: inherit;
  }
  .msg:focus,
  .feedback:focus {
    outline: 2px solid var(--l0);
    outline-offset: -1px;
  }
  .why {
    margin: 0;
    font-size: 12px;
    color: var(--fg);
  }
  .risks {
    list-style: none;
    margin: 0;
    padding: 6px 8px;
    border: 1px solid color-mix(in srgb, var(--warn) 60%, var(--line));
    border-radius: 4px;
    background: color-mix(in srgb, var(--warn) 18%, transparent);
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-size: 11.5px;
  }
  .risks li {
    display: flex;
    align-items: baseline;
    gap: 6px;
    flex-wrap: wrap;
  }
  .risk-ic {
    color: var(--warn-ink);
  }
  .risk-detail {
    color: var(--fg);
  }
  .risk-path {
    font-family: var(--mono);
    font-size: 10.5px;
    color: var(--muted);
  }
  .comments {
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding: 4px 8px;
    border-left: 2px solid var(--l0);
    font-size: 12px;
  }
  .comment-author {
    font-weight: 600;
  }
  .pad,
  .note {
    padding: 6px 10px;
    color: var(--muted);
  }
  .files {
    list-style: none;
    margin: 0;
    padding: 2px 0;
    max-height: 150px;
    overflow-y: auto;
    border-bottom: 1px solid var(--line);
    flex: none;
  }
  .frow {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 24px;
    padding: 0 10px;
  }
  .frow:hover {
    background: var(--hover);
  }
  .frow.current {
    background: var(--sel);
  }
  .fbtn {
    flex: 1 1 auto;
    min-width: 0;
    height: 100%;
    display: flex;
    align-items: center;
    cursor: default;
  }
  .fp {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .dir {
    color: var(--muted);
  }
  .diff-wrap {
    flex: 1 1 auto;
    min-height: 0;
    overflow: auto;
  }
  .actions {
    flex: none;
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 8px 10px;
    border-top: 1px solid var(--line);
    background: var(--panel);
  }
  .feedback-row {
    display: flex;
    gap: 6px;
  }
  .feedback {
    flex: 1 1 auto;
    min-width: 0;
  }
  .btn-row {
    display: flex;
    justify-content: flex-end;
    gap: 6px;
  }
  .danger {
    color: var(--bad-ink);
  }
</style>
