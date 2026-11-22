<script lang="ts" module>
  export interface ImageSide {
    label: string;
    url: string;
  }
</script>

<script lang="ts">
  let {
    before,
    after,
  }: {
    before: ImageSide;
    after: ImageSide;
  } = $props();

  type Mode = "side" | "swipe" | "onion";
  let mode = $state<Mode>("side");
  /** Swipe position or onion opacity of the "after" image, 0–100. */
  let mix = $state(50);
  /** Per URL: its pixel size once loaded, or why it didn't load. */
  let loaded = $state<Record<string, { w: number; h: number } | { error: string }>>({});

  $effect(() => {
    void before.url;
    void after.url;
    loaded = {};
  });

  function onLoad(url: string, event: Event) {
    const img = event.currentTarget as HTMLImageElement;
    loaded[url] = { w: img.naturalWidth, h: img.naturalHeight };
  }

  /** The protocol answers failures with a plain-text reason; show it. */
  async function onError(url: string) {
    loaded[url] = { error: "not in this version" };
    try {
      const response = await fetch(url);
      if (!response.ok) {
        loaded[url] = { error: await response.text() };
      }
    } catch (err) {
      console.error(`image ${url}:`, err);
    }
  }

  const both = $derived(
    [before, after].every((s) => {
      const l = loaded[s.url];
      return l && !("error" in l);
    }),
  );

  function info(url: string) {
    const l = loaded[url];
    return !l ? "loading…" : "error" in l ? l.error : `${l.w} × ${l.h}`;
  }
</script>

<section class="imgdiff" aria-label="Image diff">
  <div class="modes" role="radiogroup" aria-label="Compare">
    {#each [["side", "Side by side"], ["swipe", "Swipe"], ["onion", "Onion"]] as [m, label] (m)}
      <button
        class="bare chip"
        class:on={mode === m}
        role="radio"
        aria-checked={mode === m}
        disabled={m !== "side" && !both}
        onclick={() => (mode = m as Mode)}>{label}</button
      >
    {/each}
  </div>

  {#if mode === "side"}
    <div class="side">
      {#each [before, after] as s (s.url)}
        <figure>
          <figcaption>{s.label}</figcaption>
          <div class="frame">
            {#if !(loaded[s.url] && "error" in loaded[s.url])}
              <img src={s.url} alt={s.label} onload={(e) => onLoad(s.url, e)} onerror={() => onError(s.url)} />
            {/if}
          </div>
          <span class="ab">{info(s.url)}</span>
        </figure>
      {/each}
    </div>
  {:else}
    <div class="stack" style:--mix="{mix}%">
      <img src={before.url} alt={before.label} />
      <img
        class="top"
        class:swipe={mode === "swipe"}
        src={after.url}
        alt={after.label}
        style:opacity={mode === "onion" ? mix / 100 : 1}
      />
      {#if mode === "swipe"}<span class="line"></span>{/if}
    </div>
    <label class="slider">
      <span class="ab">{before.label}</span>
      <input type="range" min="0" max="100" bind:value={mix} aria-label={mode === "swipe" ? "Swipe position" : "Opacity of the new image"} />
      <span class="ab">{after.label}</span>
    </label>
  {/if}
</section>

<style>
  .imgdiff {
    display: grid;
    gap: 8px;
    padding: 8px 10px;
  }
  .modes {
    display: flex;
    gap: 6px;
  }
  .chip {
    all: unset;
    font-size: 11px;
    padding: 1px 9px;
    border-radius: 999px;
    border: 1px solid var(--line);
    color: var(--muted);
    cursor: default;
  }
  .chip.on {
    background: var(--sel);
    color: var(--fg);
    border-color: transparent;
  }
  .chip:disabled {
    opacity: 0.45;
  }
  .side {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 10px;
  }
  figure {
    margin: 0;
    display: grid;
    gap: 4px;
    min-width: 0;
  }
  figcaption {
    font: 600 11.5px var(--body);
  }
  .frame,
  .stack {
    display: grid;
    place-items: center;
    min-height: 80px;
    border: 1px solid var(--line);
    border-radius: 4px;
    background: repeating-conic-gradient(var(--hover) 0 25%, var(--panel) 0 50%) 0 0 / 14px 14px;
    overflow: hidden;
  }
  img {
    max-width: 100%;
    max-height: 320px;
    image-rendering: auto;
  }
  .stack {
    position: relative;
  }
  .stack img {
    grid-area: 1 / 1;
  }
  .stack .top.swipe {
    clip-path: inset(0 0 0 var(--mix));
  }
  .line {
    position: absolute;
    top: 0;
    bottom: 0;
    left: var(--mix);
    width: 2px;
    background: var(--l0);
  }
  .slider {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .slider input {
    flex: 1;
  }
</style>
