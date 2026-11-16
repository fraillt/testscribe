<script lang="ts">
  import { duration, title } from './narrative'
  import OutcomeText from './OutcomeText.svelte'
  import TimingPanel from './TimingPanel.svelte'
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

  const marks = { passed: '✓', failed: '✗', skipped: '–', not_run: '·' } as const
  const modes: { mode: RunMode; label: string; hint: string }[] = [
    { mode: 'all', label: 'everything', hint: 'Every tree that runs starts over: what the batch knew of it is forgotten.' },
    {
      mode: 'selected',
      label: 'selected tests',
      hint: 'Only the picked tests run, and the tests above them. The other tests stay as they are.',
    },
    {
      mode: 'new_and_failed',
      label: 'new and failed',
      hint: 'Only the tests that have not run yet and the ones that did not pass run. The other tests stay as they are.',
    },
  ]
  type Show = 'all' | 'failed' | 'not_run'
  const shows: { show: Show; label: string }[] = [
    { show: 'all', label: 'all' },
    { show: 'failed', label: 'not passed' },
    { show: 'not_run', label: 'not run' },
  ]

  const key = (test: TestId) => JSON.stringify([test.binary, test.path])
  const total = (test: StoredTest) => test.test_us + test.setup_us

  let show = $state<Show>('all')
  let search = $state('')
  let slowestFirst = $state(false)
  /** The test whose executions and times are shown. */
  let timed = $state<string | null>(null)
  let commandOpen = $state(false)
  /** What the command printed is shown while it runs, and after that when asked for. */
  let outputOpen = $state(false)
  let output = $state<HTMLElement>()

  let counts = $derived({
    failed: batch.tests.filter((test) => test.status === 'failed').length,
    skipped: batch.tests.filter((test) => test.status === 'skipped').length,
    not_run: batch.tests.filter((test) => test.status === 'not_run').length,
  })
  let selected = $derived(new Set(batch.selected.map(key)))
  let filtered = $derived(show !== 'all' || search.trim() !== '')
  let byTime = $derived(frontend.showTimes && slowestFirst)

  /** The tests that are asked for, and the tests above them, to tell where they are. */
  let shown = $derived.by(() => {
    if (!filtered) return batch.tests
    const words = search.toLowerCase().split(/\s+/).filter(Boolean)
    const asked = (test: StoredTest) => {
      if (show === 'failed' && test.status !== 'failed' && test.status !== 'skipped') return false
      if (show === 'not_run' && test.status !== 'not_run') return false
      const text = `${title(test.path.at(-1) ?? '')}\n${test.text}`.toLowerCase()
      return words.every((word) => text.includes(word))
    }
    const wanted = new Set<string>()
    for (const test of batch.tests) {
      if (!asked(test)) continue
      for (let depth = 1; depth <= test.path.length; depth++) {
        wanted.add(key({ binary: test.binary, path: test.path.slice(0, depth) }))
      }
    }
    return batch.tests.filter((test) => wanted.has(key(test)))
  })
  let binaries = $derived.by(() => {
    const binaries: { name: string; tests: StoredTest[] }[] = []
    for (const test of shown) {
      if (binaries.at(-1)?.name !== test.binary) binaries.push({ name: test.binary, tests: [] })
      binaries.at(-1)!.tests.push(test)
    }
    return binaries
  })
  let slowest = $derived(shown.filter((test) => test.runs > 0).toSorted((a, b) => total(b) - total(a)))
  let slowestUs = $derived(Math.max(1, ...slowest.map(total)))
  let command = $derived(
    `TESTSCRIBE_FRONTEND=${frontend.address} ` +
      (batch.stored ? `TESTSCRIBE_BATCH_ID=${/^[\w.-]+$/.test(batch.name) ? batch.name : `'${batch.name}'`} ` : '') +
      'cargo test --no-fail-fast',
  )

  // What the command prints is followed to its end.
  $effect(() => {
    if (batch.run.output.length && output) output.scrollTop = output.scrollHeight
  })

  /** The test and the tests below it: its branches. */
  function branches(test: StoredTest): StoredTest[] {
    return batch.tests.filter(
      (other) =>
        other.binary === test.binary &&
        other.path.length >= test.path.length &&
        test.path.every((name, idx) => other.path[idx] === name),
    )
  }

  /** Picks a test and the tests below it, or lets go of them. */
  function toggle(test: StoredTest, picked: boolean): void {
    const below = branches(test)
    const keys = new Set(below.map(key))
    const rest = batch.selected.filter((other) => !keys.has(key(other)))
    const now = picked ? [...rest, ...below.map(({ binary, path }) => ({ binary, path }))] : rest
    frontend.setRun(batch.name, 'selected', now)
  }

  /** A test that has not run has said nothing: it is shown by its name. */
  function lines(test: StoredTest): { text: string }[] {
    const text = test.text || `${test.path.length === 1 ? 'Given' : 'When'} ${title(test.path.at(-1) ?? '')}`
    return text.split('\n').map((text) => ({ text }))
  }

  function remove(): void {
    if (confirm(`Delete the batch "${batch.name}" and everything it knows?`)) {
      frontend.deleteBatch(batch.name)
      ondeleted()
    }
  }

  function forget(binary: string): void {
    if (confirm(`Forget what the batch knows of "${binary}"? It comes back when that test binary runs again.`)) {
      frontend.deleteBinary(batch.name, binary)
    }
  }
</script>

{#snippet row(test: StoredTest, depth: number)}
  {@const link = frontend.sourceLink(batch.dir, test.file, test.line)}
  <div class="test {test.status}" style:--depth={depth}>
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
      {#if byTime}
        <span class="where">
          {test.binary} ›
          {#each test.path.slice(0, -1) as name, nr (nr)}{title(name)} › {/each}
        </span>
      {/if}
      <OutcomeText lines={lines(test)} />
      {#if byTime}
        <span class="bar" style:width="{(total(test) / slowestUs) * 100}%"></span>
      {/if}
    </span>
    <span class="side">
      {#if link}
        <a class="source" href={link} title="open {test.file}:{test.line}">{test.file.replace(/^.*\//, '')}:{test.line}</a>
      {/if}
      {#if frontend.showTimes}
        <button
          class="time"
          class:open={timed === key(test)}
          title="executions and times of this test and its branches"
          onclick={() => (timed = timed === key(test) ? null : key(test))}
        >
          {#if test.runs}
            {#if test.runs > 1}<span class="runs">{test.runs}×</span>{/if}
            {duration(total(test))}
          {:else}–{/if}
        </button>
      {/if}
    </span>
  </div>
  {#if frontend.showTimes && timed === key(test)}
    <TimingPanel
      {depth}
      wholeTree={test.path.length === 1}
      rows={branches(test).map((branch) => ({
        name: title(branch.path.at(-1) ?? ''),
        depth: branch.path.length - test.path.length,
        runs: branch.runs,
        test_us: branch.test_us,
        setup_us: branch.setup_us,
      }))}
    />
  {/if}
{/snippet}

<header>
  <h2>{batch.name}</h2>
  <p>
    {batch.stored ? 'stored batch' : 'kept in memory only, gone when the server stops'}
    · {batch.tests.length} tests
    {#if counts.failed}· <span class="failed">{counts.failed} failed</span>{/if}
    {#if counts.skipped}· {counts.skipped} skipped{/if}
    {#if counts.not_run}· {counts.not_run} not run{/if}
    {#if batch.commit}
      · last run on {batch.branch ?? 'commit'}
      <code>{batch.commit}</code>{#if batch.dirty}&nbsp;with changes that are not committed{/if}
    {/if}
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
    {#if frontend.canRun}
      {#if batch.run.running}
        <button class="stop" onclick={() => frontend.stopBatch(batch.name)}>stop</button>
      {:else}
        <button class="run" onclick={() => frontend.runBatch(batch.name)}>▶ run</button>
      {/if}
      <button class="quiet" class:open={commandOpen} onclick={() => (commandOpen = !commandOpen)}>command…</button>
    {/if}
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

  {#if frontend.canRun && commandOpen}
    <form
      class="command"
      onsubmit={(event) => {
        event.preventDefault()
        const fields = new FormData(event.currentTarget)
        frontend.setCommand(batch.name, String(fields.get('command')), String(fields.get('dir')))
        commandOpen = false
      }}
    >
      <label>run <input name="command" value={batch.command} spellcheck="false" /></label>
      <label>in <input name="dir" value={batch.dir ?? ''} placeholder="the directory of the workspace" spellcheck="false" /></label>
      <button>save</button>
    </form>
  {/if}

  <p class="hint">
    {modes.find((item) => item.mode === batch.mode)?.hint}
    {#if frontend.canRun}
      <b>run</b> executes <code>{batch.command}</code>{#if batch.dir}&nbsp;in <code>{batch.dir}</code>{/if}. From a
      terminal:
    {:else}
      Run the tests again with
    {/if}
    <code>{command}</code>
  </p>

  {#if batch.run.output.length}
    <div class="output">
      <button class="quiet" onclick={() => (outputOpen = !outputOpen)}>
        {batch.run.running || outputOpen ? '▾' : '▸'}
        {#if batch.run.running}
          running…
        {:else if batch.run.code === 0}
          the run finished
        {:else if batch.run.code === null}
          the run was stopped
        {:else}
          the run finished with exit code {batch.run.code}
        {/if}
      </button>
      {#if batch.run.running || outputOpen}
        <pre bind:this={output}>{batch.run.output.join('\n')}</pre>
      {/if}
    </div>
  {/if}

  <div class="tools filters">
    <span class="label">show</span>
    <span class="modes">
      {#each shows as item (item.show)}
        <button class:picked={show === item.show} onclick={() => (show = item.show)}>{item.label}</button>
      {/each}
    </span>
    <input type="search" placeholder="search the tests" bind:value={search} />
    {#if frontend.showTimes}
      <label class="check"><input type="checkbox" bind:checked={slowestFirst} /> slowest first</label>
    {/if}
    {#if filtered}<span class="label">{shown.length} of {batch.tests.length} tests</span>{/if}
  </div>
</header>

{#if byTime}
  <section>
    {#each slowest as test (key(test))}
      {@render row(test, 0)}
    {:else}
      <p class="nothing">No test to show has been timed.</p>
    {/each}
  </section>
{:else}
  {#each binaries as binary (binary.name)}
    <section>
      <h3>
        {binary.name}
        <button class="quiet" title="forget what the batch knows of this test binary" onclick={() => forget(binary.name)}>
          forget
        </button>
      </h3>
      {#each binary.tests as test (key(test))}
        {@render row(test, test.path.length - 1)}
      {/each}
    </section>
  {:else}
    <p class="nothing">
      {filtered ? 'No test is like that.' : 'No test binary of this batch has reported yet.'}
    </p>
  {/each}
{/if}

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
  .delete,
  .stop {
    color: var(--bad);
  }
  .run {
    color: var(--ok);
    font-weight: 600;
  }
  .quiet {
    border: 0;
    background: none;
    color: var(--muted);
    font-size: 0.8rem;
    padding: 0.25rem 0.3rem;
  }
  .quiet:hover,
  .quiet.open {
    color: var(--text);
  }
  .command {
    display: flex;
    gap: 0.75rem;
    align-items: center;
    margin-top: 0.5rem;
    font-size: 0.85rem;
    color: var(--muted);
  }
  .command label {
    display: flex;
    gap: 0.4rem;
    align-items: center;
    flex: 1;
  }
  input:not([type='checkbox']) {
    font: inherit;
    color: var(--text);
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 5px;
    padding: 0.25rem 0.5rem;
  }
  .command input {
    flex: 1;
    font-family: var(--mono);
    font-size: 0.8rem;
  }
  .output {
    margin-bottom: 0.75rem;
  }
  .output pre {
    margin: 0.25rem 0 0;
    max-height: 16rem;
    overflow: auto;
    padding: 0.5rem 0.75rem;
    border: 1px solid var(--border);
    border-radius: 5px;
    background: var(--panel);
    font-family: var(--mono);
    font-size: 0.75rem;
    line-height: 1.4;
  }
  .filters {
    margin-bottom: 1rem;
  }
  .check {
    display: flex;
    gap: 0.35rem;
    align-items: center;
    color: var(--muted);
    cursor: pointer;
  }
  .check input {
    margin: 0;
  }
  header .hint {
    margin: 0.5rem 0 0.75rem;
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
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    margin: 0 0 0.5rem;
    padding: 0.5rem 0.75rem;
    font-size: 1rem;
    border-bottom: 1px solid var(--border);
  }
  .test {
    display: grid;
    grid-template-columns: 1.5rem 1.5rem 1fr auto;
    align-items: start;
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
  .test.skipped,
  .test.not_run {
    color: var(--muted);
  }
  .test.not_run :global(b) {
    color: var(--muted);
  }
  .side {
    display: flex;
    gap: 0.5rem;
    align-items: baseline;
    white-space: nowrap;
  }
  .source {
    color: var(--muted);
    font-size: 0.72rem;
    text-decoration: none;
    opacity: 0;
  }
  .test:hover .source,
  .source:focus {
    opacity: 1;
  }
  .source:hover {
    color: var(--accent);
    text-decoration: underline;
  }
  .where {
    display: block;
    color: var(--muted);
    font-family: system-ui, -apple-system, 'Segoe UI', sans-serif;
    font-size: 0.75rem;
  }
  .bar {
    display: block;
    height: 3px;
    margin: 0.15rem 0 0.35rem;
    border-radius: 2px;
    background: var(--accent);
    opacity: 0.6;
  }
  .pick input {
    margin: 0.3rem 0 0;
  }
  .test .time {
    border: 0;
    border-radius: 4px;
    padding: 0 0.4rem;
    background: none;
    color: var(--muted);
    font-size: 0.78rem;
    white-space: nowrap;
  }
  .test .time:hover,
  .test .time.open {
    background: var(--hover);
    color: var(--text);
  }
  .runs {
    color: var(--accent);
    margin-right: 0.3rem;
  }
  .nothing {
    color: var(--muted);
  }
</style>
