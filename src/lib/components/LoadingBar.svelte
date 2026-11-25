<script lang="ts">
  /** `wide`: a fixed width with the bar at its right end, for labels that change while it shows. */
  let { label, value = null, wide = false }: { label: string; value?: number | null; wide?: boolean } = $props();
  const pct = $derived(value === null ? null : Math.round(Math.min(1, Math.max(0, value)) * 100));
</script>

<span
  class="loading"
  class:wide
  role="progressbar"
  aria-label={label}
  aria-valuemin={pct === null ? undefined : 0}
  aria-valuemax={pct === null ? undefined : 100}
  aria-valuenow={pct ?? undefined}
>
  <span class="label" title={label}>{label}</span>
  <span class="bar" aria-hidden="true"><i class:fill={pct !== null} style:width={pct === null ? undefined : `${pct}%`}></i></span>
</span>

<style>
  /* Quick work finishes before this fades in, so it doesn't flash. */
  .loading {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    max-width: 100%;
    color: var(--fg);
    animation: show 0s 120ms both;
  }
  .label {
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    font-variant-numeric: tabular-nums;
  }
  .wide {
    width: min(360px, 100%);
  }
  .wide .label {
    flex: 1;
  }
  @keyframes show {
    from {
      visibility: hidden;
    }
  }
  .bar {
    flex: none;
    width: 120px;
    height: 5px;
    border-radius: 3px;
    background: var(--line);
    overflow: hidden;
  }
  .bar i {
    display: block;
    width: 35%;
    height: 100%;
    background: var(--l0);
    animation: sweep 1.1s ease-in-out infinite;
  }
  .bar i.fill {
    animation: none;
    transition: width 0.15s linear;
  }
  @keyframes sweep {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(290%);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .bar i {
      animation: none;
      width: 100%;
      opacity: 0.5;
    }
  }
</style>
