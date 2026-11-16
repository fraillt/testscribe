<script lang="ts">
  import { title } from './narrative'
  import OutcomeText from './OutcomeText.svelte'
  import type { Batch, BatchSummary, RunMode, StoredTest, TestId } from './protocol'
  import { frontend } from './store.svelte'

  let {
    batch,
    others,
    oncompare,
    ondeleted,
  }: {
    batch: Batch
    /** The other batches, to compare this one with. */
    others: BatchSummary[]
    oncompare: (from: string) => void
    ondeleted: () => void
  } = $props()

  const marks = { passed: '✓', failed: '✗', skipped: '–' } as const
  const modes: { mode: RunMode; label: string; hint: string }[] = [
    { mode: 'all', label: 'everything', hint: 'Every tree that runs replaces what the batch knew of it.' },
    {
      mode: 'selected',
      label: 'selected tests',
      hint: 'Only the picked tests run, and the tests above them. The other tests stay as they are.',
    },
    {
      mode: 'new_and_failed',
      label: 'new and failed',
      hint: 'Only the tests the batch does not know yet and the ones that did not pass run. The other tests stay as they are.',
    },
  ]

  const key = (test: TestId) => JSON.stringify([test.binary, test.path])

  let binaries = $derived.by(() => {
    const binaries: { name: string; tests: StoredTest[] }[] = []
    for (const test of batch.tests) {
      if (binaries.at(-1)?.name !== test.binary) binaries.push({ name: test.binary, tests: [] })
      binaries.at(-1)!.tests.push(test)
    }
    return binaries
  })
  let counts = $derived({
    passed: batch.tests.filter((test) => test.status === 'passed').length,
    failed: batch.tests.filter((test) => test.status === 'failed').length,
    skipped: batch.tests.filter((test) => test.status === 'skipped').length,
  })
  let selected = $derived(new Set(batch.selected.map(key)))
  let command = $derived(
    `TESTSCRIBE_FRONTEND=${frontend.address} ` +
      (batch.stored ? `TESTSCRIBE_BATCH_ID=${/^[\w.-]+$/.test(batch.name) ? batch.name : `'${batch.name}'`} ` : '') +
      'cargo test',
  )

  /** Picks a test and the tests below it, or lets go of them. */
  function toggle(test: StoredTest, picked: boolean): void {
    const below = batch.tests.filter(
      (other) =>
        other.binary === test.binary &&
        other.path.length >= test.path.length &&
        test.path.every((name, idx) => other.path[idx] === name),
    )
    const keys = new Set(below.map(key))
    const rest = batch.selected.filter((other) => !keys.has(key(other)))
    const now = picked ? [...rest, ...below.map(({ binary, path }) => ({ binary, path }))] : rest
    frontend.setRun(batch.name, 'selected', now)
  }

  function remove(): void {
    if (confirm(`Delete the batch "${batch.name}" and everything it knows?`)) {
      frontend.deleteBatch(batch.name)
      ondeleted()
    }
  }
</script>

<header>
  <h2>{batch.name}</h2>
  <p>
    {batch.stored ? 'stored batch' : 'kept in memory only, gone when the server stops'}
    · {batch.tests.length} tests
    {#if counts.failed}· <span class="failed">{counts.failed} failed</span>{/if}
    {#if counts.skipped}· {counts.skipped} skipped{/if}
  </p>

  <div class="tools">
    <span class="label">the next run runs</span>
    <span class="modes">
      {#each modes as item (item.mode)}
        <button
          class:picked={batch.mode === item.mode}
          title={item.hint}
          onclick={() => frontend.setRun(batch.name, item.mode)}
        >
          {item.label}{#if item.mode === 'selected' && batch.selected.length}&nbsp;({batch.selected.length}){/if}
        </button>
      {/each}
    </span>
    <span class="spacer"></span>
    {#if others.length}
      <select
        onchange={(event) => {
          if (event.currentTarget.value) oncompare(event.currentTarget.value)
        }}
      >
        <option value="">compare with…</option>
        {#each others as other (other.name)}
          <option value={other.name}>{other.name}</option>
        {/each}
      </select>
    {/if}
    <button class="delete" onclick={remove}>delete</button>
  </div>
  <p class="hint">
    {modes.find((item) => item.mode === batch.mode)?.hint} Run the tests again with
    <code>{command}</code>
  </p>
</header>

{#each binaries as binary (binary.name)}
  <section>
    <h3>{binary.name}</h3>
    {#each binary.tests as test (key(test))}
      <div class="test {test.status}" style:--depth={test.path.length - 1}>
        <span class="pick">
          {#if batch.mode === 'selected'}
            <input
              type="checkbox"
              checked={selected.has(key(test))}
              aria-label="run {title(test.path.at(-1) ?? '')}"
              onchange={(event) => toggle(test, event.currentTarget.checked)}
            />
          {/if}
        </span>
        <span class="mark">{marks[test.status]}</span>
        <span class="text">
          <OutcomeText lines={test.text.split('\n').map((text) => ({ text }))} />
        </span>
      </div>
    {/each}
  </section>
{:else}
  <p class="nothing">No test of this batch has run yet.</p>
{/each}

<style>
  h2 {
    margin: 0;
    font-size: 1.2rem;
  }
  header p {
    margin: 0.15rem 0 0.75rem;
    color: var(--muted);
    font-size: 0.8rem;
  }
  .failed {
    color: var(--bad);
  }
  .tools {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 0.5rem;
    font-size: 0.85rem;
  }
  .label {
    color: var(--muted);
  }
  .spacer {
    flex: 1;
  }
  .modes {
    display: inline-flex;
    border: 1px solid var(--border);
    border-radius: 5px;
    overflow: hidden;
  }
  button,
  select {
    font: inherit;
    color: inherit;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 5px;
    padding: 0.25rem 0.6rem;
    cursor: pointer;
  }
  .modes button {
    border: 0;
    border-radius: 0;
    border-left: 1px solid var(--border);
  }
  .modes button:first-child {
    border-left: 0;
  }
  .modes button.picked {
    background: var(--accent);
    color: var(--panel);
  }
  .delete {
    color: var(--bad);
  }
  header .hint {
    margin: 0.5rem 0 1rem;
  }
  code {
    font-family: var(--mono);
    color: var(--text);
    user-select: all;
  }
  section {
    border: 1px solid var(--border);
    border-radius: 6px;
    margin-bottom: 1rem;
    background: var(--panel);
    padding-bottom: 0.5rem;
  }
  h3 {
    margin: 0 0 0.5rem;
    padding: 0.5rem 0.75rem;
    font-size: 1rem;
    border-bottom: 1px solid var(--border);
  }
  .test {
    display: grid;
    grid-template-columns: 1.5rem 1.5rem 1fr;
    padding: 0.1rem 0.75rem;
    font-family: var(--mono);
    font-size: 0.82rem;
    line-height: 1.5;
  }
  .text {
    padding-left: calc(var(--depth) * 1.25rem);
  }
  .mark {
    text-align: center;
  }
  .test.passed .mark {
    color: var(--ok);
  }
  .test.failed .mark {
    color: var(--bad);
  }
  .test.skipped {
    color: var(--muted);
  }
  .pick input {
    margin: 0.3rem 0 0;
  }
  .nothing {
    color: var(--muted);
  }
</style>
