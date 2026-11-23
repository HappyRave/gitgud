<script lang="ts">
  import type { UnityYamlDiff } from "$lib/api";

  let {
    diff,
  }: {
    diff: UnityYamlDiff;
  } = $props();

  let filter = $state("");

  const filtered = $derived.by(() => {
    const q = filter.trim().toLowerCase();
    if (!q) {
      return diff.objects;
    }
    return diff.objects
      .map((obj) => {
        const headerMatch =
          obj.objectPath.toLowerCase().includes(q) || obj.component.toLowerCase().includes(q) || obj.fileId.includes(q);
        if (headerMatch) {
          return obj;
        }
        const matchingProps = obj.properties.filter(
          (p) =>
            p.property.toLowerCase().includes(q) ||
            (p.beforeDisplay ?? p.before ?? "").toLowerCase().includes(q) ||
            (p.afterDisplay ?? p.after ?? "").toLowerCase().includes(q),
        );
        if (matchingProps.length === 0) {
          return null;
        }
        return { ...obj, properties: matchingProps };
      })
      .filter((x): x is NonNullable<typeof x> => x !== null);
  });

  function kindLabel(kind: "added" | "removed" | "modified"): string {
    if (kind === "added") {
      return "Added";
    }
    if (kind === "removed") {
      return "Removed";
    }
    return "Modified";
  }
</script>

<div class="udiff" aria-label="Structural Unity diff for {diff.path}">
  <div class="bar">
    <span class="counts">
      <span class="c-add">+{diff.addedObjects} objects</span>
      <span class="c-rem">−{diff.removedObjects}</span>
      <span>~{diff.modifiedProperties} properties</span>
    </span>
    <input
      type="search"
      class="flt"
      bind:value={filter}
      placeholder="Filter objects or properties…"
      aria-label="Filter objects or properties"
    />
  </div>

  {#if filtered.length === 0}
    <p class="note">
      {diff.objects.length === 0 ? "No Unity YAML property changes detected." : "No matching objects or properties."}
    </p>
  {:else}
    <div class="cards">
      {#each filtered as obj (`${obj.fileId}-${obj.kind}`)}
        <section class="card">
          <header class="ch">
            <span class="badge k-{obj.kind}">{kindLabel(obj.kind)}</span>
            <strong class="opath">{obj.objectPath}</strong>
            <span class="comp">› {obj.component}</span>
            <span class="fid mono">&amp;{obj.fileId}</span>
          </header>
          {#if obj.properties.length > 0}
            <table class="ptbl">
              <tbody>
                {#each obj.properties as prop (prop.property)}
                  <tr>
                    <td class="pname mono">{prop.property}</td>
                    <td class="pval old mono" title={prop.before ?? "∅"}>{prop.beforeDisplay ?? prop.before ?? "∅"}</td>
                    <td class="arr">→</td>
                    <td class="pval new mono" title={prop.after ?? "∅"}>{prop.afterDisplay ?? prop.after ?? "∅"}</td>
                  </tr>
                {/each}
              </tbody>
            </table>
          {/if}
        </section>
      {/each}
    </div>
  {/if}
</div>

<style>
  .udiff {
    display: flex;
    flex-direction: column;
    min-height: 0;
    flex: 1;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 10px;
    border-bottom: 1px solid var(--line);
    background: var(--bg);
    font-size: 11.5px;
  }
  .counts {
    display: inline-flex;
    gap: 8px;
    color: var(--muted);
    white-space: nowrap;
  }
  .c-add {
    color: var(--l2);
  }
  .c-rem {
    color: var(--bad-ink);
  }
  .flt {
    flex: 1;
    min-width: 120px;
    height: 22px;
    font-size: 11.5px;
    padding: 0 7px;
    border: 1px solid var(--line);
    border-radius: 5px;
    background: var(--panel);
    color: var(--fg);
  }
  .flt:focus {
    outline: none;
    border-color: var(--l0);
  }
  .cards {
    flex: 1;
    overflow: auto;
    padding: 8px 10px;
    display: grid;
    gap: 8px;
    align-content: start;
  }
  .card {
    margin: 0;
    border: 1px solid var(--line);
    border-radius: 5px;
    background: var(--panel);
    overflow: hidden;
  }
  .ch {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 5px 8px;
    background: var(--bg);
    border-bottom: 1px solid var(--line);
    font-size: 12px;
  }
  .badge {
    font: 600 10px var(--mono);
    padding: 1px 5px;
    border-radius: 3px;
    text-transform: uppercase;
  }
  .k-added {
    background: var(--add);
    color: var(--add-ink);
  }
  .k-removed {
    background: var(--del);
    color: var(--bad-ink);
  }
  .k-modified {
    background: var(--sel);
    color: var(--l0);
  }
  .comp {
    color: var(--muted);
  }
  .fid {
    margin-left: auto;
    color: var(--muted);
    font-size: 11px;
  }
  .ptbl {
    width: 100%;
    border-collapse: collapse;
    font-size: 11.5px;
  }
  .ptbl td {
    padding: 3px 8px;
    border-top: 1px solid var(--line);
    vertical-align: top;
  }
  .ptbl tr:first-child td {
    border-top: 0;
  }
  .pname {
    width: 34%;
    color: var(--fg);
    word-break: break-word;
  }
  .pval {
    word-break: break-word;
  }
  .pval.old {
    color: var(--bad-ink);
    background: var(--del);
  }
  .pval.new {
    color: var(--add-ink);
    background: var(--add);
  }
  .arr {
    width: 18px;
    text-align: center;
    color: var(--muted);
  }
  .mono {
    font-family: var(--mono);
  }
  .note {
    margin: 12px 10px;
    color: var(--muted);
  }
</style>
