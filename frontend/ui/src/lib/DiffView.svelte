<script lang="ts">
  import { duration, title } from './narrative'
  import OutcomeText from './OutcomeText.svelte'
  import type { Diff, Timing } from './protocol'

  let { diff, onswap }: { diff: Diff; onswap: () => void } = $props()

  /** What is compared: what the tests say, or how long they took. */
  let tab = $state<'outcomes' | 'times'>('outcomes')

  let changes = $derived(diff.changes ?? [])
  let times = $derived(diff.times ?? [])
  const count = (kind: string) => changes.filter((change) => change.kind === kind).length
  const total = (timing: Timing) => timing.test_us + timing.setup_us
  let before = $derived(times.reduce((sum, time) => sum + total(time.from), 0))
  let after = $derived(times.reduce((sum, time) => sum + total(time.to), 0))

  function change(from: number, to: number): string {
    const by = to - from
    const percent = from > 0 ? ` (${by > 0 ? '+' : '−'}${Math.abs(Math.round((by / from) * 100))}%)` : ''
    return `${by > 0 ? '+' : by < 0 ? '−' : ''}${duration(Math.abs(by))}${percent}`
  }
</script>

<header>
  <h2>
    {diff.from} <span class="arrow">→</span>
    {diff.to}
    <button onclick={onswap} title="compare the other way round">⇄</button>
  </h2>
  {#if diff.changes === null}
    <p>One of these batches is no longer there.</p>
  {:else}
    <p>
      {#if changes.length === 0}
        Every test that ran in both batches says the same.
      {:else}
        what the tests of <b>{diff.to}</b> say differently: <span class="added">{count('added')} added</span>
        · <span class="deleted">{count('deleted')} deleted</span>
        · <span class="modified">{count('modified')} modified</span>
        {#if count('not_run')}· {count('not_run')} not run{/if}
      {/if}
    </p>
    <div class="tabs">
      <button class:picked={tab === 'outcomes'} onclick={() => (tab = 'outcomes')}>
        outcomes ({changes.length})
      </button>
      <button class:picked={tab === 'times'} onclick={() => (tab = 'times')}>times ({times.length})</button>
    </div>
  {/if}
</header>

{#if tab === 'outcomes'}
  {#each changes as change (JSON.stringify([change.binary, change.path]))}
    <section class={change.kind}>
      <h3>
        <span class="kind">{change.kind.replace('_', ' ')}</span>
        <span class="where">
          {change.binary} ›
          {#each change.path.slice(0, -1) as name, nr (nr)}{title(name)} › {/each}
        </span>
        {title(change.path.at(-1) ?? '')}
        {#if change.from && change.to && change.from !== change.to}
          <span class="status">{change.from} → {change.to}</span>
        {/if}
        {#if change.kind === 'not_run'}
          <span class="status">it ran in {diff.from}, and has not run in {diff.to}</span>
        {/if}
      </h3>
      <OutcomeText lines={change.lines} />
    </section>
  {/each}
{:else if times.length === 0}
  <p class="nothing">No test that says the same in both batches was timed in both.</p>
{:else}
  <section class="times">
    <table>
      <thead>
        <tr>
          <th></th>
          <th>{diff.from}</th>
          <th>{diff.to}</th>
          <th>change</th>
          <th>executed</th>
        </tr>
      </thead>
      <tbody>
        <tr class="sum">
          <td>all {times.length} tests that say the same in both</td>
          <td>{duration(before)}</td>
          <td>{duration(after)}</td>
          <td class:slower={after > before} class:faster={after < before}>{change(before, after)}</td>
          <td></td>
        </tr>
        {#each times as time (JSON.stringify([time.binary, time.path]))}
          {@const from = total(time.from)}
          {@const to = total(time.to)}
          <tr>
            <td>
              <span class="where">
                {time.binary} ›
                {#each time.path.slice(0, -1) as name, nr (nr)}{title(name)} › {/each}
              </span>
              {title(time.path.at(-1) ?? '')}
            </td>
            <td>{duration(from)}</td>
            <td>{duration(to)}</td>
            <td class:slower={to > from} class:faster={to < from}>{change(from, to)}</td>
            <td>
              {#if time.from.runs === time.to.runs}{time.to.runs}×{:else}{time.from.runs}× → {time.to.runs}×{/if}
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
    <p class="nothing">
      Test and setup time together, over every execution of a test. The tests that differ the most come first.
      Small differences are noise: the same code does not take the same time twice.
    </p>
  </section>
{/if}

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
  section.not_run {
    border-left-color: var(--muted);
  }
  section.not_run .kind {
    color: var(--muted);
  }
  .tabs {
    display: inline-flex;
    border: 1px solid var(--border);
    border-radius: 5px;
    overflow: hidden;
    margin-bottom: 1rem;
  }
  .tabs button {
    margin: 0;
    border: 0;
    border-radius: 0;
    padding: 0.25rem 0.6rem;
  }
  .tabs button + button {
    border-left: 1px solid var(--border);
  }
  .tabs button.picked {
    background: var(--accent);
    color: var(--panel);
  }
  .times {
    border-left-width: 1px;
    padding: 0.5rem 0.75rem;
  }
  table {
    border-collapse: collapse;
    width: 100%;
    font-size: 0.85rem;
  }
  th,
  td {
    text-align: right;
    padding: 0.2rem 0 0.2rem 1.5rem;
    font-weight: normal;
    font-family: var(--mono);
    white-space: nowrap;
  }
  th {
    color: var(--muted);
    font-family: inherit;
  }
  td:first-child {
    text-align: left;
    padding-left: 0;
    font-family: inherit;
    white-space: normal;
  }
  tr.sum td {
    font-weight: 600;
    border-bottom: 1px solid var(--border);
  }
  .slower {
    color: var(--bad);
  }
  .faster {
    color: var(--ok);
  }
  .nothing {
    color: var(--muted);
    font-size: 0.8rem;
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
