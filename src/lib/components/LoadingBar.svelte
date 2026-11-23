<script lang="ts">
  let { label }: { label: string } = $props();
</script>

<span class="loading" role="progressbar" aria-label={label}>
  {label}
  <span class="bar" aria-hidden="true"><i></i></span>
</span>

<style>
  /* Quick work finishes before this fades in, so it doesn't flash. */
  .loading {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    color: var(--fg);
    animation: show 0s 120ms both;
  }
  @keyframes show {
    from {
      visibility: hidden;
    }
  }
  .bar {
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
