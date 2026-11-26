<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { onUpdateProgress, openExternalUrl, type UpdateInfo, updateInstall } from "$lib/api";
  import Dialog from "./Dialog.svelte";

  let {
    offer,
    onskip,
    onlater,
  }: {
    offer: UpdateInfo;
    onskip: () => void;
    onlater: () => void;
  } = $props();

  let installing = $state(false);
  let percent = $state<number | null>(null);
  let error = $state<string | null>(null);
  let goButton = $state<HTMLButtonElement | null>(null);
  let stopProgress: (() => void) | undefined;

  onMount(async () => {
    goButton?.focus();
    stopProgress = await onUpdateProgress((p) => {
      percent = p.total ? Math.min(100, Math.round((p.downloaded / p.total) * 100)) : null;
    });
  });

  onDestroy(() => stopProgress?.());

  async function install() {
    error = null;
    if (offer.portable) {
      try {
        await openExternalUrl(offer.page);
        onlater();
      } catch (e) {
        error = String(e);
      }
      return;
    }
    installing = true;
    try {
      // Doesn't come back on success: the installer replaces gitgud and starts it again.
      await updateInstall();
    } catch (e) {
      error = String(e);
      installing = false;
    }
  }
</script>

<Dialog title="gitgud {offer.version} is out" width={520} layer={60} oncancel={() => !installing && onlater()}>
  <p class="ab">You have {offer.current}.{#if offer.portable} This is a portable copy: download the new one from the release page.{/if}</p>
  {#if offer.notes}
    <div class="notes">{offer.notes}</div>
  {/if}
  {#if installing}
    <div class="bar" role="progressbar" aria-valuenow={percent ?? undefined} aria-valuemin={0} aria-valuemax={100}>
      <span style:width="{percent ?? 100}%" class:busy={percent === null}></span>
    </div>
  {/if}
  {#if error}
    <p class="flag">{error}</p>
  {/if}
  {#snippet footer()}
    <button disabled={installing} onclick={onskip}>Skip this version</button>
    <span class="sp"></span>
    <button disabled={installing} onclick={onlater}>Later <span class="k">Esc</span></button>
    <button class="pri" bind:this={goButton} disabled={installing} onclick={install}>
      {#if installing}
        Installing… {percent ?? ""}{percent === null ? "" : "%"}
      {:else if offer.portable}
        Download
      {:else}
        Install and restart <span class="k">↵</span>
      {/if}
    </button>
  {/snippet}
</Dialog>

<style>
  p {
    margin: 0;
  }
  .notes {
    max-height: 240px;
    overflow: auto;
    white-space: pre-wrap;
    padding: 8px 10px;
    border: 1px solid var(--line);
    border-radius: 7px;
    background: var(--bg);
    font-size: 12px;
  }
  .bar {
    height: 6px;
    border-radius: 3px;
    background: var(--line);
    overflow: hidden;
  }
  .bar span {
    display: block;
    height: 100%;
    background: var(--l0);
    transition: width 0.2s;
  }
  .bar span.busy {
    opacity: 0.5;
  }
  .sp {
    flex: 1;
  }
</style>
