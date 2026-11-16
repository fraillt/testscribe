<script lang="ts">
  import { frontend, type Session } from './lib/store.svelte'
  import TreeView from './lib/TreeView.svelte'

  frontend.connect()

  let picked = $state<number | null>(null)
  // The newest session is shown until another one is picked.
  let session = $derived(
    frontend.sessions.find((session) => session.id === picked) ?? frontend.sessions.at(-1),
  )

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

  <nav>
    {#each frontend.sessions.toReversed() as item (item.id)}
      {@const counts = summary(item)}
      <button class:picked={item.id === session?.id} onclick={() => (picked = item.id)}>
        <span class="dot" class:failed={counts.failed > 0} class:running={item.open}></span>
        <span class="name">{item.name}</span>
        <span class="counts">
          {#if counts.failed}{counts.failed} failed ·{/if}
          {counts.passed}/{counts.total}
        </span>
      </button>
    {/each}
  </nav>

  {#if frontend.sessions.some((session) => !session.open)}
    <button class="clear" onclick={() => frontend.clear()}>clear finished</button>
  {/if}
</aside>

<main>
  {#if session}
    <header>
      <h2>{session.name}</h2>
      <p>
        {session.open ? 'running' : 'finished'}
        {#if session.hello.manifest_dir}· {session.hello.manifest_dir}{/if}
      </p>
    </header>
    {#each session.trees as tree (tree.root)}
      <TreeView {tree} />
    {/each}
  {:else}
    <div class="empty">
      <p>No tests have connected yet. Run them with the frontend named:</p>
      <pre>TESTSCRIBE_FRONTEND={frontend.address} cargo test</pre>
      <p>The tests need the <code>detached</code> feature of <code>testscribe</code>.</p>
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
    margin: 0.25rem 0 1rem;
  }
  .connection.off {
    color: var(--bad);
  }
  nav {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
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
  .dot.failed {
    background: var(--bad);
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
