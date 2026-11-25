<script lang="ts">
  let { size = 11, title }: { size?: number; title?: string } = $props();
</script>

<svg
  class="lock-icon"
  width={size}
  height={size}
  viewBox="0 0 16 16"
  role={title ? "img" : undefined}
  aria-label={title}
  aria-hidden={title ? undefined : "true"}
>
  {#if title}<title>{title}</title>{/if}
  <path d="M5 7.5V5a3 3 0 0 1 6 0v2.5" fill="none" stroke="currentColor" stroke-width="1.8" />
  <rect x="2.5" y="7" width="11" height="8.5" rx="1.6" fill="currentColor" />
</svg>

<style>
  .lock-icon {
    flex: none;
    vertical-align: -1px;
  }
</style>
