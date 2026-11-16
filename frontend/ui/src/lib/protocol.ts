// What the server sends to the UI. A session is one test binary; its hello and its events
// are the messages of the test binary as they were sent (see the `testscribe-detached` crate).

export interface FqName {
  path: string
  name: string
}

export interface TestNode {
  node: { name: FqName; tags: string[]; filename: string; line_nr: number }
  childs: TestNode[]
}

export interface Hello {
  kind: 'hello'
  protocol: number
  binary: string
  /** The batch named with `TESTSCRIBE_BATCH_ID`, if any. */
  batch?: string | null
  /** What `TESTSCRIBE_RUN` asks for, if anything. */
  run?: string | null
  /** The process that started the test binary: the same for all binaries of one `cargo test`. */
  invocation?: number | null
  manifest_dir: string | null
  cwd: string | null
  pid: number
  trees: TestNode[]
}

export interface RunInfo {
  path: FqName[]
  run_count: number
  param_info: { headers: string[]; display_str: string[] } | null
}

export type Outcome = { kind: 'Success' } | { kind: 'Failure'; details: string }

export type CheckUpdate =
  | { kind: 'Verified'; message: string; line_nr: number; file: string; outcome: Outcome }
  | {
      kind: 'ParamsStarted'
      message: string
      line_nr: number
      file: string
      columns_count: number
      header: string[] | null
    }
  | { kind: 'ParamVerified'; index: number; row: string[]; outcome: Outcome }
  | { kind: 'ParamsFinished' }

export type SkipReason =
  | { kind: 'Panicked'; name: FqName; message: string }
  | { kind: 'Ignored'; name: FqName }

export type TestUpdate =
  | { kind: 'Started'; info: RunInfo }
  | { kind: 'Updated'; info: CheckUpdate }
  | { kind: 'Finished'; panic_message: string | null }
  | { kind: 'Skipped'; info: RunInfo; reason: SkipReason }

export type TestsEvent =
  | { kind: 'tree_ready'; root: string }
  | { kind: 'test'; root: string; test: FqName; update: TestUpdate; elapsed_us: number }
  | { kind: 'tree_finished'; root: string; success: boolean }

export interface SessionData {
  id: number
  /** The batch the results of the session belong to. */
  batch: string
  /** Where the workspace of the tests is, if it is on the machine of the server. */
  workspace_dir: string | null
  hello: Hello
  events: TestsEvent[]
  open: boolean
}

/** `not_run`: the test is in the code, and nothing is known of it. */
export type Status = 'passed' | 'failed' | 'skipped' | 'not_run'

/** What a batch runs the next time its tests ask. */
export type RunMode = 'all' | 'selected' | 'new_and_failed'

export interface BatchSummary {
  name: string
  /** Whether the batch is in the database, or only in the memory of the server. */
  stored: boolean
  mode: RunMode
  updated_at: number
  passed: number
  failed: number
  skipped: number
  not_run: number
  /** Whether the server is running the tests of the batch right now. */
  running: boolean
  commit: string | null
  branch: string | null
  dirty: boolean | null
}

/** The command the server runs for a batch, and what it printed. */
export interface CommandRun {
  running: boolean
  /** How it ended; `null` while it runs, and when it was stopped. */
  code: number | null
  output: string[]
}

/** A test of some test binary: the names from the root of its tree down to it. */
export interface TestId {
  binary: string
  path: string[]
}

export interface StoredTest extends TestId {
  status: Status
  /** What the test said the last time it ran, as the terminal would print it. */
  text: string
  /** How often the test was executed in the run of its tree: again for every branch below it. */
  runs: number
  /** The time in the test itself, over all its executions, in microseconds. */
  test_us: number
  /** The time to get the test going, over all its executions: environment and copies. */
  setup_us: number
  /** Where the test is in the sources: the file, from the workspace, and the line. */
  file: string
  line: number
}

export interface Batch {
  name: string
  stored: boolean
  mode: RunMode
  selected: TestId[]
  /** What "run again" executes, and in which directory. */
  command: string
  dir: string | null
  commit: string | null
  branch: string | null
  dirty: boolean | null
  run: CommandRun
  tests: StoredTest[]
}

export interface Change extends TestId {
  /** `not_run`: the older batch has an outcome, the newer one has not run the test. */
  kind: 'added' | 'deleted' | 'modified' | 'not_run'
  from: Status | null
  to: Status | null
  lines: { side: ' ' | '+' | '-'; text: string }[]
}

export interface Diff {
  from: string
  to: string
  /** `null` when one of the batches is not there. */
  changes: Change[] | null
  /** The tests that say the same in both batches, with how they ran in each. */
  times: TimeChange[] | null
}

export interface Timing {
  runs: number
  test_us: number
  setup_us: number
}

export interface TimeChange extends TestId {
  from: Timing
  to: Timing
}

export type ServerMessage =
  | { kind: 'snapshot'; sessions: SessionData[]; batches: BatchSummary[]; can_run: boolean }
  | { kind: 'session_opened'; session: SessionData }
  | { kind: 'event'; session: number; event: TestsEvent }
  | { kind: 'session_closed'; session: number }
  | { kind: 'batches'; batches: BatchSummary[] }
  | { kind: 'batch'; name: string; batch: Batch | null }
  | ({ kind: 'diff' } & Diff)
  | { kind: 'command_output'; batch: string; lines: string[] }
  | { kind: 'command_state'; batch: string; running: boolean; code: number | null }
