// Turns the events of a test tree into what is shown of it: the same narrative the terminal
// prints, one step per test, in the order the tests ran.

import type { FqName, RunInfo, TestNode, TestsEvent } from './protocol'

export interface Check {
  kind: 'check'
  message: string
  ok: boolean
  details?: string
  file: string
  line: number
}

export interface Table {
  kind: 'table'
  message: string
  header: string[] | null
  rows: { cells: string[]; ok: boolean; details?: string }[]
  file: string
  line: number
}

export type StepStatus = 'running' | 'passed' | 'failed' | 'skipped'

/** How often a test was executed so far, and the time that took. */
export interface Timing {
  runs: number
  test_us: number
  /** The time before the test started, since the test before it ended. */
  setup_us: number
}

export interface Step {
  test: FqName
  /** The names from the root down to the test, as the key of its timing. */
  path: string
  title: string
  depth: number
  status: StepStatus
  /** The parameter the test ran with, as `name=value` pairs. */
  params: string | null
  /** A step that is not the first run of its test: shown only because it failed. */
  rerun: boolean
  startedUs: number
  setupUs: number
  durationUs: number | null
  panic: string | null
  skipped: string | null
  items: (Check | Table)[]
}

export type TreeStatus = 'not started' | 'running' | 'passed' | 'failed'

export interface Tree {
  root: string
  title: string
  outline: TestNode
  status: TreeStatus
  steps: Step[]
  /** The run in progress, when it is a test executed again: not shown unless it fails. */
  hidden: Step | null
  shownAsFailed: Set<string>
  counts: { passed: number; failed: number; skipped: number }
  /** By the path of the test, every execution counted: also the ones that are not shown. */
  timings: Record<string, Timing>
  /** When the test before the current one ended, since the tree started. */
  lastEndUs: number
}

export function pathKey(names: FqName[]): string {
  return JSON.stringify(names.map(fullName))
}

export function fullName(name: FqName): string {
  return `${name.path}::${name.name}`
}

/** `Another6IsAdded` reads `another 6 is added`, as in the terminal. */
export function title(name: string): string {
  return name
    .replace(/^.*::/, '')
    .replace(/([A-Z])/g, ' $1')
    .replace(/([^0-9 ])([0-9])/g, '$1 $2')
    .trim()
    .toLowerCase()
}

export function newTree(outline: TestNode): Tree {
  return {
    root: fullName(outline.node.name),
    title: title(outline.node.name.name),
    outline,
    status: 'not started',
    steps: [],
    hidden: null,
    shownAsFailed: new Set(),
    counts: { passed: 0, failed: 0, skipped: 0 },
    timings: {},
    lastEndUs: 0,
  }
}

function newStep(test: FqName, info: RunInfo, startedUs: number): Step {
  const param = info.param_info
  return {
    test,
    path: pathKey([...info.path, test]),
    setupUs: 0,
    title: title(test.name),
    depth: info.path.length,
    status: 'running',
    params: param ? param.headers.map((header, idx) => `${header}=${param.display_str[idx]}`).join(', ') : null,
    rerun: info.run_count > 0,
    startedUs,
    durationUs: null,
    panic: null,
    skipped: null,
    items: [],
  }
}

function current(tree: Tree): Step | undefined {
  return tree.hidden ?? tree.steps[tree.steps.length - 1]
}

export function apply(tree: Tree, event: TestsEvent): void {
  if (event.kind === 'tree_ready') {
    tree.status = 'running'
    return
  }
  if (event.kind === 'tree_finished') {
    tree.status = event.success ? 'passed' : 'failed'
    return
  }
  const update = event.update
  switch (update.kind) {
    case 'Started': {
      const step = newStep(event.test, update.info, event.elapsed_us)
      step.setupUs = Math.max(0, event.elapsed_us - tree.lastEndUs)
      if (step.rerun) tree.hidden = step
      else tree.steps.push(step)
      break
    }
    case 'Updated': {
      const step = current(tree)
      if (!step) break
      const info = update.info
      if (info.kind === 'Verified') {
        const ok = info.outcome.kind === 'Success'
        step.items.push({
          kind: 'check',
          message: info.message,
          ok,
          details: info.outcome.kind === 'Failure' ? info.outcome.details : undefined,
          file: info.file,
          line: info.line_nr,
        })
      } else if (info.kind === 'ParamsStarted') {
        step.items.push({
          kind: 'table',
          message: info.message,
          header: info.header,
          rows: [],
          file: info.file,
          line: info.line_nr,
        })
      } else if (info.kind === 'ParamVerified') {
        const table = step.items[step.items.length - 1]
        if (table?.kind === 'table') {
          table.rows.push({
            cells: info.row,
            ok: info.outcome.kind === 'Success',
            details: info.outcome.kind === 'Failure' ? info.outcome.details : undefined,
          })
        }
      }
      break
    }
    case 'Finished': {
      const step = current(tree)
      if (!step) break
      const failedCheck = step.items.some((item) =>
        item.kind === 'check' ? !item.ok : item.rows.some((row) => !row.ok),
      )
      step.panic = update.panic_message
      step.status = step.panic !== null || failedCheck ? 'failed' : 'passed'
      step.durationUs = event.elapsed_us - step.startedUs
      const timing = (tree.timings[step.path] ??= { runs: 0, test_us: 0, setup_us: 0 })
      timing.runs += 1
      timing.test_us += step.durationUs
      timing.setup_us += step.setupUs
      tree.lastEndUs = event.elapsed_us
      const key = fullName(step.test)
      if (step.rerun) {
        // A test is shown for its first run. A later run is shown only if it fails where no
        // earlier run did.
        tree.hidden = null
        if (step.status === 'failed' && !tree.shownAsFailed.has(key)) {
          tree.steps.push(step)
          tree.counts.failed += 1
          tree.counts.passed -= 1
        }
      } else if (step.status === 'failed') {
        tree.counts.failed += 1
      } else {
        tree.counts.passed += 1
      }
      if (step.status === 'failed') tree.shownAsFailed.add(key)
      break
    }
    case 'Skipped': {
      if (update.info.run_count > 0) break
      const step = newStep(event.test, update.info, event.elapsed_us)
      step.status = 'skipped'
      step.skipped =
        update.reason.kind === 'Ignored'
          ? `${title(update.reason.name.name)} is not selected`
          : update.reason.message
      tree.steps.push(step)
      tree.counts.skipped += 1
      break
    }
  }
}

export function duration(us: number): string {
  if (us < 1000) return `${us}μs`
  if (us < 1_000_000) return `${(us / 1000).toFixed(us < 10_000 ? 2 : 1)}ms`
  if (us < 60_000_000) return `${(us / 1_000_000).toFixed(2)}s`
  const seconds = Math.round(us / 1_000_000)
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, '0')}`
}
