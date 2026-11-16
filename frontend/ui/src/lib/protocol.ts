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
  hello: Hello
  events: TestsEvent[]
  open: boolean
}

export type Status = 'passed' | 'failed' | 'skipped'

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
}

export interface Batch {
  name: string
  stored: boolean
  mode: RunMode
  selected: TestId[]
  tests: StoredTest[]
}

export interface Change extends TestId {
  kind: 'added' | 'deleted' | 'modified'
  from: Status | null
  to: Status | null
  lines: { side: ' ' | '+' | '-'; text: string }[]
}

export interface Diff {
  from: string
  to: string
  /** `null` when one of the batches is not there. */
  changes: Change[] | null
}

export type ServerMessage =
  | { kind: 'snapshot'; sessions: SessionData[]; batches: BatchSummary[] }
  | { kind: 'session_opened'; session: SessionData }
  | { kind: 'event'; session: number; event: TestsEvent }
  | { kind: 'session_closed'; session: number }
  | { kind: 'batches'; batches: BatchSummary[] }
  | { kind: 'batch'; name: string; batch: Batch | null }
  | ({ kind: 'diff' } & Diff)
