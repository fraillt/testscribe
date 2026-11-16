// Everything the server has told the UI, kept up to date over one WebSocket.

import { apply, newTree, type Tree } from './narrative'
import type {
  Batch,
  BatchSummary,
  Diff,
  Hello,
  RunMode,
  ServerMessage,
  SessionData,
  TestId,
  TestsEvent,
} from './protocol'

export interface Session {
  id: number
  batch: string
  workspaceDir: string | null
  hello: Hello
  /** The name of the test binary without the hash cargo adds. */
  name: string
  open: boolean
  trees: Tree[]
}

declare global {
  interface Window {
    /** Set by the desktop shell: the address of the server it started. */
    __TESTSCRIBE_FRONTEND__?: string
  }
}

function serverAddress(): string {
  if (window.__TESTSCRIBE_FRONTEND__) return window.__TESTSCRIBE_FRONTEND__
  // The dev server of the UI is not the frontend server.
  return import.meta.env.DEV ? '127.0.0.1:7357' : window.location.host
}

function newSession(data: SessionData): Session {
  const session: Session = {
    id: data.id,
    batch: data.batch,
    workspaceDir: data.workspace_dir,
    hello: data.hello,
    name: data.hello.binary.replace(/-[0-9a-f]{16}$/, ''),
    open: data.open,
    trees: data.hello.trees.map(newTree),
  }
  for (const event of data.events) applyEvent(session, event)
  return session
}

function applyEvent(session: Session, event: TestsEvent): void {
  const tree = session.trees.find((tree) => tree.root === event.root)
  if (tree) apply(tree, event)
}

function remembered(key: string): string | null {
  try {
    return localStorage.getItem(key)
  } catch {
    return null
  }
}

function remember(key: string, value: string): void {
  try {
    localStorage.setItem(key, value)
  } catch {
    // Not remembered, then.
  }
}

/** Opens a file at a line in VS Code. `{path}` and `{line}` are filled in. */
export const DEFAULT_EDITOR = 'vscode://file/{path}:{line}'

const OUTPUT_KEPT = 2000

class Frontend {
  sessions = $state<Session[]>([])
  batches = $state<BatchSummary[]>([])
  /** The batch that is looked at, once the server has sent it. */
  batch = $state<Batch | null>(null)
  /** The comparison that is looked at, once the server has sent it. */
  diff = $state<Diff | null>(null)
  /** What is looked at: the server is asked for it again whenever the batches change. */
  #watched: { kind: 'get_batch'; name: string } | { kind: 'diff'; from: string; to: string } | null = null
  connected = $state(false)
  /** Whether the tests are shown with their executions and times. Kept by the browser. */
  showTimes = $state(remembered('testscribe.showTimes') === 'true')
  /** The link that opens a place in the sources in an editor. Kept by the browser. */
  editor = $state(remembered('testscribe.editor') ?? DEFAULT_EDITOR)
  /** Whether the server may run the tests of a batch itself. */
  canRun = $state(false)
  address = serverAddress()
  #socket: WebSocket | null = null

  connect(): void {
    const socket = new WebSocket(`ws://${this.address}/ui`)
    this.#socket = socket
    socket.onopen = () => {
      this.connected = true
      this.#ask()
    }
    socket.onmessage = (message) => this.#receive(JSON.parse(message.data))
    socket.onclose = () => {
      this.connected = false
      // The server starts every connection with all it knows, so nothing is lost by this.
      setTimeout(() => this.connect(), 1000)
    }
  }

  setShowTimes(show: boolean): void {
    this.showTimes = show
    remember('testscribe.showTimes', String(show))
  }

  setEditor(editor: string): void {
    this.editor = editor.trim() || DEFAULT_EDITOR
    remember('testscribe.editor', this.editor)
  }

  /** The link to a place in the sources, where it is known which directory they are in. */
  sourceLink(dir: string | null | undefined, file: string, line: number): string | null {
    if (!dir || !file) return null
    const path = file.startsWith('/') ? file : `${dir.replace(/\/$/, '')}/${file}`
    // `vscode://file/{path}` with a path from the root has one slash between the two.
    const editor = path.startsWith('/') ? this.editor.replace('/{path}', '{path}') : this.editor
    return editor.replace('{path}', encodeURI(path)).replace('{line}', String(line))
  }

  /** Has the server run the tests of the batch, with the command of the batch. */
  runBatch(batch: string): void {
    this.#send({ kind: 'run_batch', batch })
  }

  stopBatch(batch: string): void {
    this.#send({ kind: 'stop_batch', batch })
  }

  setCommand(batch: string, command: string, dir: string): void {
    this.#send({ kind: 'set_command', batch, command, dir })
  }

  /** Forgets what the batch knows of a test binary. */
  deleteBinary(batch: string, binary: string): void {
    this.#send({ kind: 'delete_binary', batch, binary })
  }

  /** Forgets the sessions whose test binary has exited. */
  clear(): void {
    this.#send({ kind: 'clear' })
  }

  /** Looks at a batch: its tests are kept up to date from now on. */
  watchBatch(name: string): void {
    if (this.batch?.name !== name) this.batch = null
    this.diff = null
    this.#watched = { kind: 'get_batch', name }
    this.#ask()
  }

  /** Looks at what is different in batch `to`, seen from batch `from`. */
  watchDiff(from: string, to: string): void {
    this.batch = null
    this.diff = null
    this.#watched = { kind: 'diff', from, to }
    this.#ask()
  }

  watchNothing(): void {
    this.#watched = null
  }

  /** Sets what the batch runs the next time its tests ask. */
  setRun(batch: string, mode: RunMode, selected?: TestId[]): void {
    this.#send({ kind: 'set_run', batch, mode, selected })
  }

  deleteBatch(name: string): void {
    this.#send({ kind: 'delete_batch', name })
  }

  #send(command: object): void {
    if (this.#socket?.readyState === WebSocket.OPEN) this.#socket.send(JSON.stringify(command))
  }

  #ask(): void {
    if (this.#watched) this.#send(this.#watched)
  }

  #receive(message: ServerMessage): void {
    switch (message.kind) {
      case 'snapshot':
        this.sessions = message.sessions.map(newSession)
        this.batches = message.batches
        this.canRun = message.can_run
        break
      case 'command_output':
        if (this.batch?.name === message.batch) {
          const output = this.batch.run.output
          output.push(...message.lines)
          if (output.length > OUTPUT_KEPT) output.splice(0, output.length - OUTPUT_KEPT)
        }
        break
      case 'command_state':
        if (this.batch?.name === message.batch) {
          if (message.running) this.batch.run.output = []
          this.batch.run.running = message.running
          this.batch.run.code = message.code
        }
        break
      case 'batches':
        this.batches = message.batches
        this.#ask()
        break
      case 'batch':
        if (this.#watched?.kind === 'get_batch' && this.#watched.name === message.name) {
          this.batch = message.batch
        }
        break
      case 'diff':
        if (
          this.#watched?.kind === 'diff' &&
          this.#watched.from === message.from &&
          this.#watched.to === message.to
        ) {
          this.diff = { from: message.from, to: message.to, changes: message.changes, times: message.times }
        }
        break
      case 'session_opened':
        this.sessions.push(newSession(message.session))
        break
      case 'event': {
        const session = this.sessions.find((session) => session.id === message.session)
        if (session) applyEvent(session, message.event)
        break
      }
      case 'session_closed': {
        const session = this.sessions.find((session) => session.id === message.session)
        if (session) session.open = false
        break
      }
    }
  }
}

export const frontend = new Frontend()
