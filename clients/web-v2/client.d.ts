/** Local Bridge Web SDK V2: build-time, version-pinned browser client. */
export type BridgeTransport = 'direct' | 'native-fallback' | 'unavailable'
export interface BridgeSession {
  endpoint: 'http://127.0.0.1:48293'
  token: string
  expires_in: number
  scope?: string
  transport?: string
}
export interface BridgeCapabilities {
  protocol?: string
  local_bridge?: boolean
  workbuddy?: {
    installed?: boolean
    local_grant?: boolean
    run_supported?: boolean
    ready?: boolean
  }
  mcp?: unknown
  [key: string]: unknown
}
export interface BridgeConnection {
  available: boolean
  mode: BridgeTransport
  session: BridgeSession | null
  capabilities: BridgeCapabilities | null
}
export type BridgeEvent =
  | {seq: number; type: 'message.delta'; message_id: string; text: string}
  | {seq: number; type: 'message.final'; message_id: string}
  | {seq: number; type: 'tool.update'; state?: string}
  | {seq: number; type: 'task.started' | 'task.completed' | 'task.failed' | 'task.unknown'; code?: string}
  | {type: 'task.status'; status: string; terminal: true}
export interface BridgeTaskPage {
  task_id: string
  status: string
  terminal: boolean
  next_seq: number
  events: BridgeEvent[]
  has_more: boolean
  transport?: BridgeTransport
}
export declare const CLIENT_VERSION: string
export declare const WEB_CHANNEL: string
export declare class LocalBridgeError extends Error {
  code: string
  constructor(code: string)
}
export declare function chromeRequest(action: string, params?: Record<string, unknown>,
  timeout?: number, page?: Window): Promise<unknown>
export declare function directJson(session: BridgeSession, path: string,
  options?: {method?: string; body?: string; signal?: AbortSignal},
  fetchImpl?: typeof fetch): Promise<unknown>
export declare function directStream(session: BridgeSession, taskId: string,
  cursor: number, onEvent: (event: BridgeEvent) => void, signal?: AbortSignal,
  fetchImpl?: typeof fetch): Promise<number>
export declare class LocalBridgeWebClient {
  constructor(options?: {page?: Window; fetchImpl?: typeof fetch})
  readonly mode: BridgeTransport
  readonly session: BridgeSession | null
  readonly capabilities: BridgeCapabilities | null
  connect(): Promise<BridgeConnection>
  disconnect(): void
  startWorkbuddy(task: {clientKey: string; prompt: string}): Promise<{
    task_id: string; status: string; reused?: boolean; provider?: string
  }>
  pollWorkbuddy(taskId: string, cursor?: number): Promise<BridgeTaskPage>
  mcpStatus(provider: string, tool: string): Promise<unknown>
  readWorkbuddy(taskId: string, cursor?: number,
    onEvent?: (event: BridgeEvent) => void, signal?: AbortSignal): Promise<BridgeTaskPage>
}
export declare function createLocalBridgeClient(
  options?: ConstructorParameters<typeof LocalBridgeWebClient>[0]
): LocalBridgeWebClient
