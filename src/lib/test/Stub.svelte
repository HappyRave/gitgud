<script lang="ts">
  import type { Snippet } from "svelte";

  let props: { path?: string; chrome?: Snippet; opener?: unknown } & Record<string, unknown> = $props();
</script>

<div data-testid="stub" data-path={props.path} data-built={props.opener ? "yes" : "no"}>
  {#if props.chrome}{@render props.chrome()}{/if}
</div>
