<script lang="ts">
  import { title } from './narrative'
  import OutcomeText from './OutcomeText.svelte'
  import type { Diff } from './protocol'

  let { diff, onswap }: { diff: Diff; onswap: () => void } = $props()

  let changes = $derived(diff.changes ?? [])
  const count = (kind: string) => changes.filter((change) => change.kind === kind).length
</script>

<header>
  <h2>
    {diff.from} <span class="arrow">→</span>
    {diff.to}
    <button onclick={onswap} title="compare the other way round">⇄</button>
  </h2>
  {#if diff.changes === null}
    <p>One of these batches is no longer there.</p>
  {:else if changes.length === 0}
    <p>Every test says the same in both batches.</p>
  {:else}
    <p>
      what the tests of <b>{diff.to}</b> say differently: <span class="added">{count('added')} added</span>
      · <span class="deleted">{count('deleted')} deleted</span>
      · <span class="modified">{count('modified')} modified</span>
    </p>
  {/if}
</header>

{#each changes as change (JSON.stringify([change.binary, change.path]))}
  <section class={change.kind}>
    <h3>
      <span class="kind">{change.kind}</span>
      <span class="where">
        {change.binary} ›
        {#each change.path.slice(0, -1) as name, nr (nr)}{title(name)} › {/each}
      </span>
      {title(change.path.at(-1) ?? '')}
      {#if change.from && change.to && change.from !== change.to}
        <span class="status">{change.from} → {change.to}</span>
      {/if}
    </h3>
    <OutcomeText lines={change.lines} />
  </section>
{/each}

<style>
  h2 {
    margin: 0;
    font-size: 1.2rem;
  }
  .arrow {
    color: var(--muted);
  }
  header p {
    margin: 0.15rem 0 1rem;
    color: var(--muted);
    font-size: 0.8rem;
  }
  button {
    font: inherit;
    font-size: 0.85rem;
    color: inherit;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 5px;
    padding: 0.1rem 0.5rem;
    margin-left: 0.5rem;
    cursor: pointer;
    vertical-align: middle;
  }
  header .added,
  section.added .kind {
    color: var(--ok);
  }
  header .deleted,
  section.deleted .kind {
    color: var(--bad);
  }
  header .modified,
  section.modified .kind {
    color: var(--accent);
  }
  section {
    border: 1px solid var(--border);
    border-left: 4px solid var(--accent);
    border-radius: 6px;
    margin-bottom: 1rem;
    background: var(--panel);
    padding-bottom: 0.5rem;
  }
  section.added {
    border-left-color: var(--ok);
  }
  section.deleted {
    border-left-color: var(--bad);
  }
  h3 {
    margin: 0 0 0.5rem;
    padding: 0.5rem 0.75rem;
    font-size: 0.95rem;
    border-bottom: 1px solid var(--border);
  }
  .kind {
    font-size: 0.7rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    margin-right: 0.5rem;
  }
  .where,
  .status {
    color: var(--muted);
    font-weight: normal;
  }
  .status {
    font-size: 0.8rem;
    margin-left: 0.5rem;
  }
  section :global(pre) {
    padding: 0 0.75rem;
  }
</style>
