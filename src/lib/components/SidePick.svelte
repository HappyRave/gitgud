<script lang="ts">
  import { conflictSides, type ConflictSides, type MergeSide } from "$lib/api";
  import { sideKey } from "$lib/keys";
  import { imageUrl, isImage } from "$lib/images";
  import { formatSize } from "$lib/merge/resolve";

  let {
    repoId,
    path,
    revision,
    yoursLabel,
    theirsLabel,
    busy = false,
    onpick,
  }: {
    repoId: number;
    path: string;
    revision: number;
    yoursLabel: string;
    theirsLabel: string;
    busy?: boolean;
    onpick: (side: MergeSide) => void;
  } = $props();

  let sides = $state<ConflictSides | null>(null);
  let error = $state<string | null>(null);

  $effect(() => {
    const key = `${repoId}:${path}:${revision}`;
    conflictSides(repoId, path).then(
      (s) => {
        if (key === `${repoId}:${path}:${revision}`) {
          sides = s;
          error = null;
        }
      },
      (err) => (error = String(err)),
    );
  });

  const deletion = $derived(!!sides && (!sides.yours || !sides.theirs));

  function onKeydown(event: KeyboardEvent) {
    const side = sideKey(event);
    if (side && sides && !busy) {
      event.preventDefault();
      onpick(side);
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

<section class="pick" aria-label="Pick a side for {path}">
  <div class="ph">
    <span class="name">{path}</span>
    <span class="ct">· {deletion ? "deleted on one side" : "binary"} · can't be merged, pick a side</span>
  </div>
  {#if error}
    <p role="alert" class="pad">{error}</p>
  {:else if sides}
    <div class="cards">
      {#each [{ side: "yours", label: `Yours · ${yoursLabel}`, info: sides.yours, key: "1" }, { side: "theirs", label: `Theirs · ${theirsLabel}`, info: sides.theirs, key: "2" }] as card (card.side)}
        <div class="cd {card.side}">
          <b>{card.label}</b>
          {#if card.info}
            {#if isImage(path)}
              <div class="preview">
                <img src={imageUrl(repoId, path, { at: card.side as "yours" | "theirs" }, revision)} alt="{card.label} preview" />
              </div>
            {/if}
            <span class="what">
              {formatSize(card.info.size)}
              {#if card.info.lfs}<span class="badge">LFS</span>{/if}
            </span>
          {:else}
            <span class="what gone">Deleted on this side</span>
          {/if}
          <button disabled={busy} onclick={() => onpick(card.side as MergeSide)}>
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
    background: var(--panel);
  }
  .name {
    text-transform: none;
    letter-spacing: 0;
    color: var(--fg);
  }
  .pad {
    padding: 0 16px;
    margin: 0;
  }
  .cards {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 300px));
    gap: 16px;
    padding: 16px;
  }
  .cd {
    display: grid;
    gap: 10px;
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
  .preview {
    display: grid;
    place-items: center;
    width: 100%;
    aspect-ratio: 1;
    border: 1px solid var(--line);
    border-radius: 4px;
    background: repeating-conic-gradient(var(--hover) 0 25%, var(--panel) 0 50%) 0 0 / 14px 14px;
    overflow: hidden;
  }
  .preview img {
    max-width: 100%;
    max-height: 100%;
  }
  .what {
    display: flex;
    gap: 6px;
    align-items: center;
    font: 600 20px var(--mono);
  }
  .what.gone {
    font: italic 13px var(--body);
    color: var(--muted);
  }
</style>
