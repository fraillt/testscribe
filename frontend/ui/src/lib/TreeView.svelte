<script lang="ts">
  import { duration, title, type Tree } from './narrative'
  import type { TestNode } from './protocol'

  let { tree }: { tree: Tree } = $props()

  const marks = { running: '…', passed: '✓', failed: '✗', skipped: '–' } as const
</script>

{#snippet outline(node: TestNode, depth: number)}
  <div class="step pending" style:--depth={depth}>
    <span class="mark">·</span>
    <span class="time"></span>
    <span class="text"><b>{depth === 0 ? 'Given' : 'When'}</b> {title(node.node.name.name)}</span>
  </div>
  {#each node.childs as child (child.node.name.name)}
    {@render outline(child, depth + 1)}
  {/each}
{/snippet}

<section class="tree {tree.status.replace(' ', '-')}">
  <header>
    <span class="status">{tree.status}</span>
    <h2>{tree.title}</h2>
    <span class="counts">
      {#if tree.counts.failed}<span class="failed">{tree.counts.failed} failed</span>{/if}
      {#if tree.counts.passed}<span>{tree.counts.passed} passed</span>{/if}
      {#if tree.counts.skipped}<span>{tree.counts.skipped} skipped</span>{/if}
    </span>
  </header>

  <div class="narrative">
    {#if tree.steps.length === 0}
      {@render outline(tree.outline, 0)}
    {/if}
    {#each tree.steps as step, idx (idx)}
      <div class="step {step.status}" style:--depth={step.depth} title={step.skipped ?? ''}>
        <span class="mark">{marks[step.status]}</span>
        <span class="time">{step.durationUs === null ? '' : duration(step.durationUs)}</span>
        <span class="text">
          <b>{step.depth === 0 ? 'Given' : 'When'}</b>
          {step.title}
          {#if step.rerun}<i>(executed again)</i>{/if}
        </span>
      </div>
      {#if step.params}
        <div class="line" style:--depth={step.depth}>
          <span class="mark"></span><span class="time"></span>
          <span class="text"><b>With</b> {step.params}</span>
        </div>
      {/if}
      {#if step.panic}
        <div class="line failed" style:--depth={step.depth + 1}>
          <span class="mark"></span><span class="time"></span>
          <span class="text"><pre>{step.panic}</pre></span>
        </div>
      {/if}
      {#each step.items as item, nr (nr)}
        {#if item.kind === 'check'}
          <div class="line" class:failed={!item.ok} style:--depth={step.depth + 1}>
            <span class="mark">{item.ok ? '' : '✗'}</span><span class="time"></span>
            <span class="text">
              <b>{nr === 0 ? 'Then' : 'And'}</b>
              {item.message}
              {#if !item.ok}
                <pre>{item.details}</pre>
                <small>{item.file}:{item.line}</small>
              {/if}
            </span>
          </div>
        {:else}
          {@const failed = item.rows.some((row) => !row.ok)}
          <div class="line" class:failed style:--depth={step.depth + 1}>
            <span class="mark">{failed ? '✗' : ''}</span><span class="time"></span>
            <span class="text">
              <b>{nr === 0 ? 'Then' : 'And'}</b>
              {item.message}
              <table>
                {#if item.header}
                  <thead>
                    <tr>{#each item.header as cell (cell)}<th>{cell}</th>{/each}</tr>
                  </thead>
                {/if}
                <tbody>
                  {#each item.rows as row, rowNr (rowNr)}
                    <tr class:failed={!row.ok}>
                      {#each row.cells as cell, cellNr (cellNr)}<td>{cell}</td>{/each}
                      {#if !row.ok}<td class="details">{row.details}</td>{/if}
                    </tr>
                  {/each}
                </tbody>
              </table>
              {#if failed}<small>{item.file}:{item.line}</small>{/if}
            </span>
          </div>
        {/if}
      {/each}
    {/each}
  </div>
</section>

<style>
  .tree {
    border: 1px solid var(--border);
    border-left: 4px solid var(--muted);
    border-radius: 6px;
    margin-bottom: 1rem;
    background: var(--panel);
  }
  .tree.passed {
    border-left-color: var(--ok);
  }
  .tree.failed {
    border-left-color: var(--bad);
  }
  .tree.running {
    border-left-color: var(--accent);
  }
  header {
    display: flex;
    align-items: baseline;
    gap: 0.75rem;
    padding: 0.5rem 0.75rem;
    border-bottom: 1px solid var(--border);
  }
  h2 {
    font-size: 1rem;
    margin: 0;
    flex: 1;
  }
  .status {
    font-size: 0.7rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--muted);
    min-width: 5.5rem;
  }
  .passed .status {
    color: var(--ok);
  }
  .failed .status {
    color: var(--bad);
  }
  .running .status {
    color: var(--accent);
  }
  .counts {
    display: flex;
    gap: 0.75rem;
    font-size: 0.8rem;
    color: var(--muted);
  }
  .counts .failed {
    color: var(--bad);
  }
  .narrative {
    padding: 0.5rem 0;
    font-family: var(--mono);
    font-size: 0.82rem;
    line-height: 1.5;
  }
  .step,
  .line {
    display: grid;
    grid-template-columns: 1.5rem 4.5rem 1fr;
    padding: 0 0.75rem;
  }
  .step {
    margin-top: 0.15rem;
  }
  .text {
    padding-left: calc(var(--depth) * 1.25rem);
    overflow-wrap: anywhere;
  }
  .time {
    color: var(--muted);
    text-align: right;
    padding-right: 0.75rem;
  }
  .mark {
    text-align: center;
  }
  b {
    color: var(--keyword);
    font-weight: 600;
  }
  i {
    color: var(--muted);
  }
  .step.passed .mark {
    color: var(--ok);
  }
  .step.failed,
  .line.failed,
  tr.failed {
    color: var(--bad);
  }
  .step.failed b,
  .line.failed b {
    color: var(--bad);
  }
  .step.skipped,
  .step.pending {
    color: var(--muted);
  }
  .step.skipped b,
  .step.pending b {
    color: var(--muted);
  }
  .step.running .mark {
    color: var(--accent);
  }
  pre {
    margin: 0.15rem 0;
    white-space: pre-wrap;
    font: inherit;
  }
  small {
    color: var(--muted);
  }
  table {
    border-collapse: collapse;
    margin: 0.15rem 0 0.3rem;
  }
  th,
  td {
    text-align: right;
    padding: 0 0.75rem 0 0;
    font-weight: normal;
  }
  th {
    color: var(--muted);
  }
  td.details {
    text-align: left;
  }
</style>
