<script lang="ts">
  import { type BinaryMeta, type BinarySide, type UnityAssetInfo, unityAssetInfo } from "$lib/api";
  import type { ModelSide } from "$lib/model/open";
  import { toVersion } from "$lib/model/source";
  import { formatSize } from "$lib/merge/resolve";
  import { setViewMode } from "$lib/prefs.svelte";
  import { metaTarget, viewerFor } from "$lib/viewers/route";
  import LoadingBar from "./LoadingBar.svelte";
  import ModelViewer from "./ModelViewer.svelte";
  import PreviewCard from "./PreviewCard.svelte";

  let {
    repoId,
    path,
    before,
    after,
    bust = 0,
    meta = null,
    onfetch,
  }: {
    repoId: number;
    path: string;
    /** The earlier version, or null for a new file. */
    before: ModelSide | null;
    after: ModelSide;
    bust?: number;
    meta?: BinaryMeta | null;
    onfetch?: (side: "before" | "after", meta: BinarySide) => Promise<void>;
  } = $props();

  /** Unity class ids with a visual view: 43 Mesh opens in 3D. */
  const MESH = 43;

  const target = $derived(metaTarget(path) ?? path);
  const kind = $derived(viewerFor(path));
  /** Unity text formats whose contents decide the view, or that may be stored binary. */
  const probes = $derived(kind === "asset" || kind === "material" || kind === "curves" || kind === "scene");

  let info = $state<{ before: UnityAssetInfo | null; after: UnityAssetInfo | null } | null>(null);
  let probeError = $state<string | null>(null);

  $effect(() => {
    const key = `${repoId}:${target}:${JSON.stringify(before?.at)}:${JSON.stringify(after.at)}:${bust}`;
    info = null;
    probeError = null;
    if (!probes) {
      return;
    }
    const read = (side: ModelSide | null) =>
      side ? unityAssetInfo(repoId, target, toVersion(side.at)).catch(() => null) : Promise.resolve(null);
    Promise.all([read(before), read(after)]).then(
      ([b, a]) => {
        if (key === `${repoId}:${target}:${JSON.stringify(before?.at)}:${JSON.stringify(after.at)}:${bust}`) {
          info = { before: b, after: a };
        }
      },
      (err) => (probeError = String(err)),
    );
  });

  const binary = $derived(!!info && (info.before?.binary || info.after?.binary));
  const shown = $derived(
    kind === "asset" ? (info ? (info.after?.classId ?? info.before?.classId) === MESH ? "model" : null : "probing") : kind,
  );
  const sizes = $derived(
    info ? [info.before?.size, info.after?.size].filter((s): s is number => s != null).map(formatSize).join(" → ") : "",
  );
  const openSides = $derived([...(before ? [before] : []), after]);
</script>

<div class="preview">
  {#if metaTarget(path)}
    <p class="note" role="note">
      <b>Import settings changed</b> in <code>{path.slice(path.lastIndexOf("/") + 1)}</code>; the asset is shown at both
      versions with each version's settings.
    </p>
  {/if}
  {#if probeError}
    <p role="alert" class="note">{probeError}</p>
  {:else if probes && !info}
    <p class="loading"><LoadingBar label="Reading the file…" /></p>
  {:else if binary}
    <PreviewCard
      {repoId}
      path={target}
      title="Stored in Unity's binary format"
      lines={[
        "Asset Serialization isn't Force Text for this file, so its contents can't be read or compared. Settings › Unity Setup checks the project's serialization.",
        `Size ${sizes}`,
      ]}
      sides={openSides}
    />
  {:else if shown === "model"}
    <ModelViewer {repoId} path={target} {before} {after} {bust} {meta} {onfetch} />
  {:else}
    <div class="none">
      <p>No visual view for this kind of asset.</p>
      <button type="button" onclick={() => setViewMode(path, "structure")}>Show Structure</button>
    </div>
  {/if}
</div>

<style>
  .preview {
    display: flex;
    flex-direction: column;
    min-height: 0;
    flex: 1;
  }
  .preview > :global(:last-child) {
    flex: 1;
    min-height: 0;
  }
  .note {
    margin: 6px 10px 0;
    padding: 5px 9px;
    border-radius: 5px;
    background: var(--warn);
    color: var(--warn-ink);
    font-size: 11.5px;
  }
  .note code {
    font: 11px var(--mono);
  }
  .loading {
    margin: 12px 10px;
    font-size: 11.5px;
  }
  .none {
    display: grid;
    justify-items: start;
    gap: 8px;
    padding: 14px 12px;
    color: var(--muted);
    font-size: 12px;
  }
  .none p {
    margin: 0;
  }
</style>
