<script lang="ts">
  // How a test and its branches ran: executions and times, summed up.
  import { duration } from './narrative'

  export interface TimingRow {
    name: string
    /** How far below the test the panel is about. */
    depth: number
    runs: number
    test_us: number
    setup_us: number
  }

  let {
    rows,
    wholeTree,
    depth = 0,
    inset = 3.75,
  }: {
    rows: TimingRow[]
    wholeTree: boolean
    depth?: number
    /** Where the text of a test starts in the list the panel is in, in rem. */
    inset?: number
  } = $props()

  let total = $derived({
    runs: rows.reduce((sum, row) => sum + row.runs, 0),
    test_us: rows.reduce((sum, row) => sum + row.test_us, 0),
    setup_us: rows.reduce((sum, row) => sum + row.setup_us, 0),
  })
</script>

<div class="timing" style:--depth={depth} style:--inset="{inset}rem">
  <table>
    <thead>
      <tr><th></th><th>executed</th><th>test</th><th>setup</th><th>total</th></tr>
    </thead>
    <tbody>
      {#each rows as row, nr (nr)}
        <tr>
          <td style:padding-left="{row.depth * 1.25}rem">{row.name}</td>
          <td>{row.runs}×</td>
          <td>{duration(row.test_us)}</td>
          <td>{duration(row.setup_us)}</td>
          <td>{duration(row.test_us + row.setup_us)}</td>
        </tr>
      {/each}
    </tbody>
    {#if rows.length > 1}
      <tfoot>
        <tr>
          <td>with its branches</td>
          <td>{total.runs}×</td>
          <td>{duration(total.test_us)}</td>
          <td>{duration(total.setup_us)}</td>
          <td>{duration(total.test_us + total.setup_us)}</td>
        </tr>
      </tfoot>
    {/if}
  </table>
  <p>
    A test is executed again for every branch below it, unless what it built can be copied. Setup is the time
    before a test starts: its environment is created, and what it starts from is copied.
    {#if wholeTree}Test and setup time together are the time of the whole tree.{/if}
  </p>
</div>

<style>
  .timing {
    margin: 0.25rem 0.75rem 0.6rem calc(var(--inset) + var(--depth) * 1.25rem);
    padding: 0.5rem 0.75rem;
    border: 1px solid var(--border);
    border-radius: 5px;
    background: var(--bg);
    font-family: system-ui, -apple-system, 'Segoe UI', sans-serif;
    font-size: 0.8rem;
  }
  table {
    border-collapse: collapse;
    font-family: var(--mono);
  }
  th,
  td {
    text-align: right;
    padding: 0.05rem 0 0.05rem 1.5rem;
    font-weight: normal;
  }
  th {
    color: var(--muted);
  }
  td:first-child {
    text-align: left;
    padding-right: 1rem;
  }
  tfoot td {
    border-top: 1px solid var(--border);
    font-weight: 600;
  }
  p {
    margin: 0.5rem 0 0;
    color: var(--muted);
    max-width: 44rem;
  }
</style>
