<script lang="ts">
  let props: { path?: string } & Record<string, unknown> = $props();
</script>

<div data-testid="stub" data-path={props.path}></div>
