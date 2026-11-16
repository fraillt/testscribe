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
  hello: Hello
  events: TestsEvent[]
  open: boolean
}

export type ServerMessage =
  | { kind: 'snapshot'; sessions: SessionData[] }
  | { kind: 'session_opened'; session: SessionData }
  | { kind: 'event'; session: number; event: TestsEvent }
  | { kind: 'session_closed'; session: number }
