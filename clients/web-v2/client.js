/**
 * Local Bridge Web Client V2 (dependency-free ESM).
 * Build/pin this source in a trusted application. Never fetch/execute JS from
 * localhost or from a remote ApplicationProfile at runtime.
 *
 * Chrome: one-time trusted Origin + session bootstrap, compatibility fallback.
 * Bridge: primary HTTP / SSE data plane and MCP/Agent task authority.
 */
export const CLIENT_VERSION = '2.0.0-preview.2'
export const WEB_CHANNEL = 'local-tool-bridge:web:v2'
const LOOPBACK = 'http://127.0.0.1:48293'
const DEFAULT_TIMEOUT = 3500
const TASK_ID = /^wb-[a-f0-9-]{1,92}$/
const CLIENT_KEY = /^[A-Za-z0-9_.-]{1,80}$/
const TOKEN = /^[a-f0-9]{64}$/

export class LocalBridgeError extends Error {
  constructor(code) {
    super(code)
    this.name = 'LocalBridgeError'
    this.code = code
  }
}

function requireClientKey(value) {
  if (typeof value !== 'string' || !CLIENT_KEY.test(value))
    throw new LocalBridgeError('invalid_client_key')
  return value
}
function requireTaskId(value) {
  if (typeof value !== 'string' || !TASK_ID.test(value))
    throw new LocalBridgeError('invalid_task_id')
  return value
}
function requireCursor(value) {
  if (!Number.isSafeInteger(value) || value < 0 || value > 100000)
    throw new LocalBridgeError('invalid_cursor')
  return value
}
function requireSession(session) {
  if (!session || session.endpoint !== LOOPBACK ||
    typeof session.token !== 'string' || !TOKEN.test(session.token) ||
    !Number.isSafeInteger(session.expires_in) ||
    session.expires_in < 1 || session.expires_in > 600)
    throw new LocalBridgeError('invalid_local_session')
  return session
}
function urlFor(session, path) {
  requireSession(session)
  if (typeof path !== 'string' || path.length > 512 ||
    !path.startsWith('/') || path.startsWith('//') || path.includes('..'))
    throw new LocalBridgeError('invalid_local_path')
  const url = new URL(path, session.endpoint)
  if (url.origin !== LOOPBACK) throw new LocalBridgeError('invalid_local_origin')
  return url.toString()
}
function validMcp(provider, tool) {
  return typeof provider === 'string' && /^[A-Za-z0-9._-]{1,64}$/.test(provider) &&
    typeof tool === 'string' && /^(?:status|[A-Za-z0-9._-]{1,56}_status)$/.test(tool)
}

const AGENTS = new Set(['auto','workbuddy','codex','pi'])
const THINK_MODES = new Set(['auto','off','low','medium','high','xhigh','max'])

function agentRequest(options) {
  if (!options || typeof options !== 'object' || Array.isArray(options))
    throw new LocalBridgeError('invalid_agent_options')
  if (Object.keys(options).some(k =>
    !['clientKey','prompt','agent','model','thinkMode','agentMode'].includes(k)))
    throw new LocalBridgeError('invalid_agent_options')
  const {clientKey, prompt, agent='auto', model='auto',
    thinkMode='auto', agentMode='prefer'} = options
  requireClientKey(clientKey)
  if (typeof prompt !== 'string' || !prompt.trim() ||
      new TextEncoder().encode(prompt).length > 12000)
    throw new LocalBridgeError('invalid_agent_prompt')
  if (!AGENTS.has(agent) ||
      !['prefer','require'].includes(agentMode) ||
      (agent === 'auto' && agentMode === 'require') ||
      typeof model !== 'string' || model.length < 1 || model.length > 128 ||
      !/^[A-Za-z0-9_./:-]+$/.test(model) ||
      model.includes('://') || model.startsWith('/') ||
      !THINK_MODES.has(thinkMode))
    throw new LocalBridgeError('invalid_agent_options')
  return {
    client_key:clientKey, prompt, agent, model,
    think_mode:thinkMode, agent_mode:agentMode
  }
}

/** Chrome ContentScript/ServiceWorker handles sender.origin; page cannot choose it. */
export function chromeRequest(action, params = {}, timeout = 15000, page = window) {
  return new Promise((resolve, reject) => {
    const id = 'lb' + Date.now().toString(36) + Math.random().toString(36).slice(2, 12)
    let timer
    const finish = () => {
      clearTimeout(timer)
      page.removeEventListener('message', listener)
    }
    function listener(event) {
      if (event.source !== page || event.origin !== page.location.origin) return
      const msg = event.data
      if (!msg || msg.channel !== WEB_CHANNEL || msg.type !== 'RESPONSE' ||
        msg.version !== 2 || msg.requestId !== id) return
      finish()
      if (msg.ok) resolve(msg.result || {})
      else reject(new LocalBridgeError(msg.code || msg.error || 'native_unavailable'))
    }
    page.addEventListener('message', listener)
    timer = setTimeout(() => {
      finish()
      reject(new LocalBridgeError('native_request_timeout'))
    }, timeout)
    page.postMessage({channel: WEB_CHANNEL, version: 2, type: 'REQUEST',
      requestId: id, action, params}, page.location.origin)
  })
}

export async function directJson(session, path, options = {}, fetchImpl = fetch) {
  const response = await fetchImpl(urlFor(session, path), {
    method: options.method || 'GET',
    headers: {
      Authorization: 'Bearer ' + session.token,
      ...(options.body ? {'Content-Type': 'application/json'} : {})
    },
    ...(options.body ? {body: options.body} : {}),
    ...(options.signal ? {signal: options.signal} : {}),
    credentials: 'omit', mode: 'cors', redirect: 'error',
    referrerPolicy: 'no-referrer', cache: 'no-store'
  })
  if (!response.ok) throw new LocalBridgeError('local_http_' + response.status)
  const data = await response.json()
  if (!data || typeof data !== 'object' || Array.isArray(data))
    throw new LocalBridgeError('invalid_local_response')
  return data
}

/** One bounded connection; caller keeps seq and reconnects to receive more. */
export async function directStream(session, taskId, cursor, onEvent,
  signal, fetchImpl = fetch) {
  requireTaskId(taskId)
  requireCursor(cursor)
  const controller = new AbortController()
  const abort = () => controller.abort()
  if (signal) signal.addEventListener('abort', abort, {once: true})
  const timeout = setTimeout(abort, 22000)
  let seen = cursor
  try {
    const response = await fetchImpl(urlFor(session,
      '/v2/workbuddy/tasks/' + taskId + '/stream?cursor=' + cursor), {
      method: 'GET',
      headers: {Authorization: 'Bearer ' + session.token},
      credentials: 'omit', mode: 'cors', redirect: 'error',
      cache: 'no-store', referrerPolicy: 'no-referrer',
      signal: controller.signal
    })
    if (!response.ok || !response.body)
      throw new LocalBridgeError('local_stream_' + response.status)
    const reader = response.body.getReader()
    const decoder = new TextDecoder('utf-8')
    let buf = ''
    try {
      while (true) {
        const {done, value} = await reader.read()
        if (done) break
        buf += decoder.decode(value, {stream: true})
        if (buf.length > 150000) throw new LocalBridgeError('local_stream_frame_too_large')
        let stop
        while ((stop = buf.indexOf('\n\n')) >= 0) {
          const frame = buf.slice(0, stop)
          buf = buf.slice(stop + 2)
          if (frame.startsWith(':')) continue
          const data = frame.split('\n').find(x => x.startsWith('data: '))
          if (!data) continue
          const event = JSON.parse(data.slice(6))
          if (Number.isSafeInteger(event.seq)) {
            if (event.seq <= seen) continue
            if (event.seq !== seen + 1) throw new LocalBridgeError('local_event_gap')
            seen = event.seq
          } else if (!(event.type === 'task.status' && event.terminal === true)) {
            throw new LocalBridgeError('invalid_event_seq')
          }
          onEvent(event)
        }
      }
    } finally {
      try { await reader.cancel() } catch (e) { /* connection closed */ }
    }
  } finally {
    clearTimeout(timeout)
    if (signal) signal.removeEventListener('abort', abort)
  }
  return seen
}

/**
 * Task transport owned by Local Bridge. The client never persists any Bearer
 * or injects it into URLs, and never retries a submission with a new key.
 */
export class LocalBridgeWebClient {
  constructor({page, fetchImpl} = {}) {
    this.page = page || (typeof window === 'undefined' ? null : window)
    this.fetchImpl = fetchImpl || ((...args) => fetch(...args))
    this.session = null
    this.mode = 'unavailable'
    this.capabilities = null
  }
  requestChrome(action, params, timeout) {
    if (!this.page) return Promise.reject(new LocalBridgeError('browser_required'))
    return chromeRequest(action, params, timeout, this.page)
  }
  async connect() {
    // Failure is feature discovery failure, never failure of the enterprise page.
    let session = null
    try {
      session = requireSession(await this.requestChrome('local.connect', {}, DEFAULT_TIMEOUT))
      const capabilities = await directJson(session,
        '/v2/capabilities', {}, this.fetchImpl)
      if (capabilities.protocol !== 'local-bridge-http-sse-v2')
        throw new LocalBridgeError('unsupported_bridge_protocol')
      this.session = session // in memory only
      this.mode = 'direct'
      this.capabilities = capabilities
      return {available: true, mode: 'direct', session, capabilities}
    } catch (e) {
      this.session = null
    }
    try {
      const capabilities = await this.requestChrome('local.capabilities', {}, DEFAULT_TIMEOUT)
      this.capabilities = capabilities
      this.mode = 'native-fallback'
      return {available: true, mode: 'native-fallback', session: null, capabilities}
    } catch (e) {
      this.mode = 'unavailable'
      this.capabilities = null
      return {available: false, mode: 'unavailable', session: null, capabilities: null}
    }
  }
  disconnect() {
    this.session = null
    this.mode = 'unavailable'
    this.capabilities = null
  }
  /**
   * Provider-neutral Agent API. Bridge selects the actual ready Provider.
   * agent is a preference, unless agentMode=require. An explicit model or
   * thinkMode must be honored or rejected by Bridge, never silently ignored.
   * The current Web runtime executes WorkBuddy only; other adapters follow.
   */
  async runAgent(options) {
    const req = agentRequest(options)
    if (this.session && this.mode === 'direct') {
      try {
        return await directJson(this.session, '/v2/agents/tasks',
          {method:'POST', body:JSON.stringify(req)}, this.fetchImpl)
      } catch (e) {
        // Server rejections must never be converted into a new Agent request
        // through another channel. Only transport breakage may use fallback,
        // preserving the exact same client key and options.
        if (e.code && e.code.startsWith('local_http_')) throw e
        this.mode = 'native-fallback'
      }
    }
    return this.requestChrome('agent.run', req)
  }
  // Multi-event reads currently use WorkBuddy task IDs. Keep the semantic
  // alias until additional adapters and their event types are available.
  async readAgent(taskId, cursor = 0, onEvent = () => {}, signal) {
    return this.readWorkbuddy(taskId, cursor, onEvent, signal)
  }
  async startWorkbuddy({clientKey, prompt}) {
    requireClientKey(clientKey)
    if (typeof prompt !== 'string' || !prompt.trim() ||
      new TextEncoder().encode(prompt).length > 12000)
      throw new LocalBridgeError('invalid_agent_prompt')
    if (this.session && this.mode === 'direct') {
      try {
        return await directJson(this.session, '/v2/workbuddy/tasks',
          {method: 'POST', body: JSON.stringify({client_key: clientKey, prompt})},
          this.fetchImpl)
      } catch (e) {
        // One logical submission; fallback keeps the exact same idempotency key.
        if (e.code === 'local_http_400' || e.code === 'local_http_403')
          throw e
        this.mode = 'native-fallback'
      }
    }
    return this.requestChrome('agent.start',
      {provider: 'workbuddy', client_key: clientKey, prompt})
  }
  async pollWorkbuddy(taskId, cursor = 0) {
    requireTaskId(taskId)
    requireCursor(cursor)
    if (this.session && this.mode === 'direct') {
      try {
        return await directJson(this.session,
          '/v2/workbuddy/tasks/' + taskId + '/events?cursor=' + cursor,
          {}, this.fetchImpl)
      } catch (e) {
        if (e.code === 'local_http_403' || e.code === 'local_http_404')
          throw e
        this.mode = 'native-fallback'
      }
    }
    return this.requestChrome('agent.poll', {task_id: taskId, cursor})
  }
  async mcpStatus(provider, tool) {
    if (!validMcp(provider, tool)) throw new LocalBridgeError('invalid_mcp_status')
    if (this.session && this.mode === 'direct') {
      try {
        return await directJson(this.session, '/v2/mcp/status',
          {method: 'POST', body: JSON.stringify({provider, tool})}, this.fetchImpl)
      } catch (e) {
        if (e.code === 'local_http_403' || e.code === 'local_http_400')
          throw e
        this.mode = 'native-fallback'
      }
    }
    return this.requestChrome('mcp.status', {provider, tool})
  }
  /** Direct SSE if possible; otherwise one cursor page over Native.
   * The caller controls retry interval and cancellation, avoiding a hidden
   * always-on browser timer or a second task state engine.
   */
  async readWorkbuddy(taskId, cursor = 0, onEvent = () => {}, signal) {
    requireTaskId(taskId)
    requireCursor(cursor)
    let next = cursor
    if (this.session && this.mode === 'direct') {
      let terminal = false
      let status = 'running'
      try {
        await directStream(this.session, taskId, cursor, e => {
          // Preserve the last delivered seq even if the stream disconnects
          // halfway through a burst. Chrome fallback must not replay chunks.
          if (Number.isSafeInteger(e.seq) && e.seq > next) next = e.seq
          if (e.type === 'task.status' && e.terminal) {
            status = e.status || 'unknown'
            terminal = true
          } else {
            if (e.type === 'task.completed') {status = 'completed'; terminal = true}
            if (e.type === 'task.failed') {status = 'failed'; terminal = true}
          }
          onEvent(e)
        }, signal, this.fetchImpl)
        return {task_id: taskId, status, terminal, next_seq: next,
          events: [], has_more: false, transport: 'direct'}
      } catch (e) {
        if (signal && signal.aborted) throw e
        this.mode = 'native-fallback'
      }
    }
    const result = await this.pollWorkbuddy(taskId, next)
    if (result.task_id !== taskId || !Number.isSafeInteger(result.next_seq) ||
      result.next_seq < next || !Array.isArray(result.events))
      throw new LocalBridgeError('invalid_task_events')
    let seen = next
    for (const event of result.events) {
      if (!Number.isSafeInteger(event.seq) || event.seq !== seen + 1)
        throw new LocalBridgeError('local_event_gap')
      onEvent(event)
      seen = event.seq
    }
    return {...result, transport: 'native-fallback'}
  }
}
export function createLocalBridgeClient(options) {
  return new LocalBridgeWebClient(options)
}
