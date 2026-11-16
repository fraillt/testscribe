<script lang="ts">
  // What a test said, line by line, as the server keeps it. With `side`, a line of a
  // comparison: only one of the two batches has it, or both.
  let { lines }: { lines: { text: string; side?: ' ' | '+' | '-' }[] } = $props()

  function parts(text: string) {
    const found = /^(✗ |– | {2})?(Given|When|With|Then|And)( .*)?$/.exec(text)
    if (!found) return { mark: '', keyword: '', rest: text }
    return { mark: found[1] ?? '', keyword: found[2], rest: found[3] ?? '' }
  }
</script>

<pre>{#each lines as line, nr (nr)}{@const part = parts(line.text)}<span
      class="line"
      class:failed={line.text.startsWith('✗')}
      class:added={line.side === '+'}
      class:deleted={line.side === '-'}
      >{#if line.side !== undefined}<span class="side">{line.side}</span>{/if}{part.mark}<b
        >{part.keyword}</b
      >{part.rest}
</span>{/each}</pre>

<style>
  pre {
    margin: 0;
    font-family: var(--mono);
    font-size: 0.82rem;
    line-height: 1.5;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .line {
    display: block;
  }
  b {
    color: var(--keyword);
    font-weight: 600;
  }
  .failed,
  .failed b {
    color: var(--bad);
  }
  .side {
    display: inline-block;
    width: 1.5ch;
    color: var(--muted);
    user-select: none;
  }
  .added {
    background: color-mix(in srgb, var(--ok) 14%, transparent);
  }
  .deleted {
    background: color-mix(in srgb, var(--bad) 14%, transparent);
  }
</style>
