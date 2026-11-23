<script lang="ts">
  import { conflictSides, lfsFetchObject, type ConflictSides, type LfsLock, type MergeSide, type SideInfo } from "$lib/api";
  import { formatSizeDelta, imageFormatLabel, imageUrl, isImage } from "$lib/images";
  import { sideKey } from "$lib/keys";
  import { formatSize } from "$lib/merge/resolve";
  import ImageDiff from "./ImageDiff.svelte";

  let {
    repoId,
    path,
    revision,
    yoursLabel,
    theirsLabel,
    busy = false,
    lock = null,
    onpick,
  }: {
    repoId: number;
    path: string;
    revision: number;
    yoursLabel: string;
    theirsLabel: string;
    busy?: boolean;
    lock?: LfsLock | null;
    onpick: (side: MergeSide) => void;
  } = $props();

  let sides = $state<ConflictSides | null>(null);
  let error = $state<string | null>(null);
  let fetchRev = $state(0);
  let fetching = $state<{ yours: boolean; theirs: boolean }>({ yours: false, theirs: false });
  let fetchError = $state<{ yours: string | null; theirs: string | null }>({ yours: null, theirs: null });

  $effect(() => {
    const key = `${repoId}:${path}:${revision}`;
    fetchRev = 0;
    fetching = { yours: false, theirs: false };
    fetchError = { yours: null, theirs: null };
    conflictSides(repoId, path).then(
      (s) => {
        if (key === `${repoId}:${path}:${revision}`) {
          sides = s;
          error = null;
        }
      },
      (err) => {
        if (key === `${repoId}:${path}:${revision}`) {
          error = String(err);
        }
      },
    );
  });

  const deletion = $derived(!!sides && (!sides.yours || !sides.theirs));
  const anyLfs = $derived(Boolean(sides?.yours?.lfs || sides?.theirs?.lfs));
  const fmtLabel = $derived(imageFormatLabel(path));
  const conflictKindLabel = $derived(deletion ? "deleted on one side" : anyLfs ? "LFS binary" : "binary");
  const theirsDelta = $derived(formatSizeDelta(sides?.yours?.size, sides?.theirs?.size));

  function onKeydown(event: KeyboardEvent) {
    const side = sideKey(event);
    if (side && sides && !busy) {
      event.preventDefault();
      onpick(side);
    }
  }

  async function fetchSide(side: "before" | "after" | "yours" | "theirs", info: Pick<SideInfo, "size" | "oid">) {
    const which: MergeSide = side === "before" || side === "yours" ? "yours" : "theirs";
    if (!info.oid) {
      return;
    }
    const currentRepo = repoId;
    const currentPath = path;
    await lfsFetchObject(currentRepo, info.oid, info.size, currentPath);
    if (repoId !== currentRepo || path !== currentPath) {
      return;
    }
    fetchRev += 1;
    sides = await conflictSides(currentRepo, currentPath);
  }

  async function runInlineFetch(side: MergeSide, info: SideInfo) {
    if (fetching[side] || busy) {
      return;
    }
    fetching = { ...fetching, [side]: true };
    fetchError = { ...fetchError, [side]: null };
    try {
      await fetchSide(side, info);
    } catch (err) {
      fetchError = { ...fetchError, [side]: String(err) };
    } finally {
      fetching = { ...fetching, [side]: false };
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

<section class="pick" aria-label="Pick a side for {path}">
  <div class="ph">
    <span class="name">{path}</span>
    {#if fmtLabel}
      <span class="tag">{fmtLabel}</span>
    {/if}
    <span class="ct">· {conflictKindLabel} · can't be merged, pick a side</span>
    {#if lock}
      <span class="lockpill" class:theirs={!lock.ours} title="Locked by {lock.owner} since {lock.lockedAt}">
        🔒 {lock.ours ? "Locked by you" : `Locked by ${lock.owner}`}
      </span>
    {/if}
  </div>
  {#if error}
    <p role="alert" class="pad">{error}</p>
  {:else if sides}
    {#if isImage(path) && (sides.yours || sides.theirs)}
      <div class="viewer">
        <ImageDiff
          {path}
          before={{ label: `Yours · ${yoursLabel}`, url: imageUrl(repoId, path, { at: "yours" }, revision + fetchRev) }}
          after={{ label: `Theirs · ${theirsLabel}`, url: imageUrl(repoId, path, { at: "theirs" }, revision + fetchRev) }}
          beforeMeta={sides.yours}
          afterMeta={sides.theirs}
          onfetch={(which, meta) => fetchSide(which, meta)}
        />
      </div>
    {/if}
    <div class="cards">
      {#each [{ side: "yours" as const, label: `Yours · ${yoursLabel}`, info: sides.yours, key: "1", delta: null }, { side: "theirs" as const, label: `Theirs · ${theirsLabel}`, info: sides.theirs, key: "2", delta: theirsDelta }] as card (card.side)}
        <div class="cd {card.side}">
          <b>{card.label}</b>
          {#if card.info}
            {@const info = card.info}
            <span class="what">
              <span>{formatSize(info.size)}</span>
              {#if card.delta}
                <span class="delta">{card.delta}</span>
              {/if}
              {#if info.lfs}
                {@const hydrated = info.hydrated !== false}
                <span class="badge lfs">LFS</span>
                <span class="pill" class:ok={hydrated} class:ptr={!hydrated}>
                  {hydrated ? "Hydrated" : "Pointer only"}
                </span>
              {/if}
            </span>
            {#if info.oid}
              <span class="oid mono" title="sha256:{info.oid}">OID {info.oid.slice(0, 12)}</span>
            {/if}
            {#if info.lfs && info.hydrated === false && info.oid}
              <button
                type="button"
                class="fetchbtn"
                disabled={busy || fetching[card.side]}
                onclick={() => runInlineFetch(card.side, info)}
              >
                {fetching[card.side] ? "Fetching…" : `Fetch LFS object (${formatSize(info.size)})`}
              </button>
              {#if fetchError[card.side]}
                <span class="ferr" role="alert">{fetchError[card.side]}</span>
              {/if}
            {/if}
          {:else}
            <span class="what gone">Deleted on this side</span>
          {/if}
          <button disabled={busy} onclick={() => onpick(card.side)}>
            <span class="k">{card.key}</span>
            {card.info ? "Keep this one" : "Keep it deleted"}
          </button>
        </div>
      {/each}
    </div>
    <p class="ab pad">
      {deletion
        ? "One branch deleted the file while the other changed it: keep the changed file or the deletion."
        : "Binary files can't be merged line by line. The side you keep is staged; Ctrl+Z undoes the pick."}
    </p>
  {/if}
</section>

<style>
  .pick {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow-y: auto;
    background: var(--panel);
  }
  .name {
    text-transform: none;
    letter-spacing: 0;
    color: var(--fg);
  }
  .tag {
    font: 700 10px var(--mono);
    padding: 1px 5px;
    border-radius: 3px;
    background: var(--hover);
    color: var(--fg);
  }
  .lockpill {
    margin-left: auto;
    font: 600 11px var(--body);
    padding: 1px 7px;
    border-radius: 999px;
    background: var(--sel);
    color: var(--l0);
  }
  .lockpill.theirs {
    background: var(--warn);
    color: var(--warn-ink);
  }
  .viewer {
    border-bottom: 1px solid var(--line);
    background: var(--bg);
  }
  .pad {
    padding: 0 16px 14px;
    margin: 0;
  }
  .cards {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 320px));
    gap: 16px;
    padding: 16px;
  }
  .cd {
    display: grid;
    gap: 8px;
    justify-items: start;
    padding: 14px;
    border: 1px solid var(--line);
    border-radius: 8px;
  }
  .cd.yours b {
    color: var(--yours);
  }
  .cd.theirs b {
    color: var(--theirs);
  }
  .what {
    display: flex;
    gap: 6px;
    align-items: center;
    flex-wrap: wrap;
    font: 600 18px var(--mono);
  }
  .what.gone {
    font: italic 13px var(--body);
    color: var(--muted);
  }
  .delta {
    font: 600 11px var(--mono);
    color: var(--muted);
  }
  .badge.lfs {
    font: 700 10px var(--mono);
    padding: 1px 5px;
    border-radius: 3px;
    background: var(--sel);
    color: var(--l0);
  }
  .pill {
    font: 600 10px var(--mono);
    padding: 1px 6px;
    border-radius: 999px;
    background: var(--hover);
    color: var(--muted);
  }
  .pill.ok {
    background: var(--add);
    color: var(--add-ink);
  }
  .pill.ptr {
    background: var(--warn);
    color: var(--warn-ink);
  }
  .oid {
    font-size: 11px;
    color: var(--muted);
  }
  .mono {
    font-family: var(--mono);
  }
  .fetchbtn {
    height: 24px;
    padding: 0 9px;
    font-size: 11.5px;
  }
  .ferr {
    color: var(--bad-ink);
    font-size: 11px;
  }
</style>
