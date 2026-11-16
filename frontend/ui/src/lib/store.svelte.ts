// Everything the server has told the UI, kept up to date over one WebSocket.

import { apply, newTree, type Tree } from './narrative'
import type { Hello, ServerMessage, SessionData, TestsEvent } from './protocol'

export interface Session {
  id: number
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

class Frontend {
  sessions = $state<Session[]>([])
  connected = $state(false)
  address = serverAddress()
  #socket: WebSocket | null = null

  connect(): void {
    const socket = new WebSocket(`ws://${this.address}/ui`)
    this.#socket = socket
    socket.onopen = () => (this.connected = true)
    socket.onmessage = (message) => this.#receive(JSON.parse(message.data))
    socket.onclose = () => {
      this.connected = false
      // The server starts every connection with all it knows, so nothing is lost by this.
      setTimeout(() => this.connect(), 1000)
    }
  }

  /** Forgets the sessions whose test binary has exited. */
  clear(): void {
    this.#socket?.send(JSON.stringify({ kind: 'clear' }))
  }

  #receive(message: ServerMessage): void {
    switch (message.kind) {
      case 'snapshot':
        this.sessions = message.sessions.map(newSession)
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
