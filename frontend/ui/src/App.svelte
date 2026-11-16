<script lang="ts">
  import BatchView from './lib/BatchView.svelte'
  import DiffView from './lib/DiffView.svelte'
  import { frontend, type Session } from './lib/store.svelte'
  import TreeView from './lib/TreeView.svelte'

  frontend.connect()

  type View =
    | { kind: 'session'; id: number | null }
    | { kind: 'batch'; name: string }
    | { kind: 'diff'; from: string; to: string }

  let view = $state<View>({ kind: 'session', id: null })
  // The newest session is shown until something else is picked.
  let session = $derived(
    view.kind === 'session'
      ? (frontend.sessions.find((session) => view.kind === 'session' && session.id === view.id) ??
          frontend.sessions.at(-1))
      : undefined,
  )

  function show(next: View): void {
    view = next
    if (next.kind === 'batch') frontend.watchBatch(next.name)
    else if (next.kind === 'diff') frontend.watchDiff(next.from, next.to)
    else frontend.watchNothing()
  }

  /** The test binaries of one `cargo test` belong together. */
  function invocation(session: Session): string {
    return `${session.batch} ${session.hello.invocation ?? session.id}`
  }

  /** A batch that has not run yet: it is run from its page, once it knows where. */
  function newBatch(): void {
    const name = prompt('The name of the new batch:')?.trim()
    if (!name) return
    if (!frontend.batches.some((batch) => batch.name === name)) frontend.setCommand(name, '', '')
    show({ kind: 'batch', name })
  }

  function summary(session: Session) {
    const failed = session.trees.filter((tree) => tree.status === 'failed').length
    const passed = session.trees.filter((tree) => tree.status === 'passed').length
    const running = session.trees.filter((tree) => tree.status === 'running').length
    return { failed, passed, running, total: session.trees.length }
  }
</script>

<aside>
  <h1>testscribe</h1>
  <p class="connection" class:off={!frontend.connected}>
    {frontend.connected ? 'listening on' : 'no server at'}
    {frontend.address}
  </p>

  <label class="option">
    <input
      type="checkbox"
      checked={frontend.showTimes}
      onchange={(event) => frontend.setShowTimes(event.currentTarget.checked)}
    />
    show times
  </label>
  <details class="option">
    <summary>open sources with</summary>
    <input
      value={frontend.editor}
      spellcheck="false"
      title="the link that opens a file in your editor; {'{path}'} and {'{line}'} are filled in"
      onchange={(event) => frontend.setEditor(event.currentTarget.value)}
    />
  </details>

  {#if frontend.batches.length}
    <h4>batches</h4>
    <nav>
      {#each frontend.batches as item (item.name)}
        <button
          class:picked={(view.kind === 'batch' && view.name === item.name) ||
            (view.kind === 'diff' && (view.from === item.name || view.to === item.name))}
          onclick={() => show({ kind: 'batch', name: item.name })}
        >
          <span
            class="dot"
            class:failed={item.failed > 0}
            class:unknown={item.passed + item.failed === 0}
            class:running={item.running}
          ></span>
          <span class="name">{item.name}</span>
          <span class="counts">
            {#if item.failed}{item.failed} failed ·{/if}
            {item.passed}/{item.passed + item.failed + item.skipped + item.not_run}
            {#if item.running}· running{/if}
            {#if !item.stored}· not stored{/if}
            {#if item.mode === 'selected'}· runs selected{/if}
            {#if item.mode === 'new_and_failed'}· runs new and failed{/if}
          </span>
        </button>
      {/each}
    </nav>
  {/if}

  {#if frontend.canRun}
    <button class="clear new" onclick={newBatch}>new batch…</button>
  {/if}

  {#if frontend.sessions.length}
    <h4>runs</h4>
  {/if}
  <nav class="runs">
    {#each frontend.sessions.toReversed() as item, nr (item.id)}
      {@const counts = summary(item)}
      {@const before = frontend.sessions.toReversed()[nr - 1]}
      {#if nr > 0 && invocation(before) !== invocation(item)}<hr />{/if}
      <button class:picked={item.id === session?.id} onclick={() => show({ kind: 'session', id: item.id })}>
        <span class="dot" class:failed={counts.failed > 0} class:running={item.open}></span>
        <span class="name">{item.name}</span>
        <span class="counts">
          {#if counts.failed}{counts.failed} failed ·{/if}
          {counts.passed}/{counts.total} · {item.batch}
        </span>
      </button>
    {/each}
  </nav>

  {#if frontend.sessions.some((session) => !session.open)}
    <button class="clear" onclick={() => frontend.clear()}>clear finished</button>
  {/if}
</aside>

<main>
  {#if view.kind === 'batch'}
    {@const name = view.name}
    {#if frontend.batch}
      <BatchView
        batch={frontend.batch}
        others={frontend.batches.filter((other) => other.name !== name)}
        oncompare={(from) => show({ kind: 'diff', from, to: name })}
        ondeleted={() => show({ kind: 'session', id: null })}
      />
    {:else}
      <div class="empty"><p>There is no batch {name} (any more).</p></div>
    {/if}
  {:else if view.kind === 'diff'}
    {@const { from, to } = view}
    {#if frontend.diff}
      <DiffView diff={frontend.diff} onswap={() => show({ kind: 'diff', from: to, to: from })} />
    {/if}
  {:else if session}
    <header>
      <h2>{session.name}</h2>
      <p>
        {session.open ? 'running' : 'finished'}
        · batch
        <button class="link" onclick={() => show({ kind: 'batch', name: session.batch })}>{session.batch}</button>
        {#if session.hello.manifest_dir}· {session.hello.manifest_dir}{/if}
      </p>
    </header>
    {#each session.trees as tree (tree.root)}
      <TreeView {tree} workspaceDir={session.workspaceDir} />
    {/each}
  {:else}
    <div class="empty">
      <p>No tests have connected yet. Run them with the frontend named:</p>
      <pre>TESTSCRIBE_FRONTEND={frontend.address} cargo test</pre>
      <p>The tests need the <code>detached</code> feature of <code>testscribe</code>.</p>
      <p>
        With <code>TESTSCRIBE_BATCH_ID=&lt;name&gt;</code> the results are stored as a batch of that name, which
        can be compared with other batches.
      </p>
    </div>
  {/if}
</main>

<style>
  aside {
    width: 17rem;
    flex: none;
    border-right: 1px solid var(--border);
    background: var(--panel);
    padding: 1rem 0.75rem;
    display: flex;
    flex-direction: column;
    overflow-y: auto;
  }
  h1 {
    font-size: 1.1rem;
    margin: 0;
  }
  .connection {
    font-size: 0.75rem;
    color: var(--ok);
    margin: 0.25rem 0 0.75rem;
  }
  .option {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0 0.5rem;
    margin-bottom: 0.5rem;
    font-size: 0.8rem;
    color: var(--muted);
    cursor: pointer;
  }
  .option input {
    margin: 0;
  }
  details.option {
    display: block;
  }
  details.option input {
    width: 100%;
    box-sizing: border-box;
    margin-top: 0.3rem;
    font-family: var(--mono);
    font-size: 0.72rem;
    color: var(--text);
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 0.2rem 0.35rem;
  }
  nav hr {
    width: 100%;
    border: 0;
    border-top: 1px solid var(--border);
    margin: 0.25rem 0;
  }
  .connection.off {
    color: var(--bad);
  }
  h4 {
    margin: 0.5rem 0 0.25rem;
    padding: 0 0.5rem;
    font-size: 0.7rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--muted);
  }
  nav {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
  }
  nav.runs {
    flex: 1;
  }
  nav button {
    display: grid;
    grid-template-columns: 1rem 1fr;
    text-align: left;
    border: 0;
    border-radius: 5px;
    padding: 0.4rem 0.5rem;
    background: none;
    color: inherit;
    font: inherit;
    cursor: pointer;
  }
  nav button:hover,
  nav button.picked {
    background: var(--hover);
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .counts {
    grid-column: 2;
    font-size: 0.75rem;
    color: var(--muted);
  }
  .dot {
    width: 0.5rem;
    height: 0.5rem;
    border-radius: 50%;
    background: var(--ok);
    align-self: center;
  }
  .dot.running {
    background: var(--accent);
  }
  .dot.unknown {
    background: var(--muted);
  }
  .dot.failed {
    background: var(--bad);
  }
  .link {
    border: 0;
    padding: 0;
    background: none;
    font: inherit;
    color: var(--accent);
    cursor: pointer;
  }
  .clear.new {
    margin: 0.35rem 0 0.25rem;
  }
  .clear {
    border: 1px solid var(--border);
    border-radius: 5px;
    background: none;
    color: var(--muted);
    padding: 0.3rem;
    font: inherit;
    font-size: 0.8rem;
    cursor: pointer;
  }
  main {
    flex: 1;
    overflow-y: auto;
    padding: 1rem 1.5rem;
  }
  main header h2 {
    margin: 0;
    font-size: 1.2rem;
  }
  main header p {
    margin: 0.15rem 0 1rem;
    color: var(--muted);
    font-size: 0.8rem;
  }
  .empty {
    max-width: 34rem;
    margin: 4rem auto;
    color: var(--muted);
  }
  .empty pre {
    font-family: var(--mono);
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 5px;
    padding: 0.75rem;
    color: var(--text);
  }
</style>
