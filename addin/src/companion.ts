/**
 * Talks to the SlideRelay companion app running on this computer.
 *
 * Why plain http://127.0.0.1 works from an https:// task pane: browsers treat
 * the loopback address as "potentially trustworthy", so it isn't blocked as
 * mixed content. The companion only listens on 127.0.0.1 (never the network)
 * and every command needs a secret token that the teacher approves once
 * ("pairing").
 */
import {
  API_VERSION,
  CONTROL_BASE,
  type Command,
  type Hello,
  type PairPoll,
  type PairStart,
  type ServerMessage,
  type Status,
} from '@sliderelay/shared/protocol';

const TOKEN_KEY = 'sliderelay.token';

/** Why we can't talk to the companion right now. */
export type ConnectionProblem =
  | 'not-running' // nothing answered on the port
  | 'companion-outdated' // the app is older than this add-in understands
  | 'addin-outdated' // this add-in is older than the app accepts
  | 'not-paired'; // the app is fine but we don't have a valid token

export class CompanionError extends Error {
  constructor(public readonly problem: ConnectionProblem) {
    super(problem);
  }
}

// ---------- Token storage ----------

function loadToken(): string | null {
  try {
    return localStorage.getItem(TOKEN_KEY);
  } catch {
    return null; // storage can be unavailable in some webviews
  }
}

function saveToken(token: string | null): void {
  try {
    if (token) localStorage.setItem(TOKEN_KEY, token);
    else localStorage.removeItem(TOKEN_KEY);
  } catch {
    /* ignore — we'll just pair again next time */
  }
}

// ---------- HTTP helpers ----------

async function request<T>(path: string, init: RequestInit = {}, timeoutMs = 3000): Promise<T> {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  let response: Response;
  try {
    const headers = new Headers(init.headers);
    const token = loadToken();
    if (token) headers.set('Authorization', `Bearer ${token}`);
    response = await fetch(CONTROL_BASE + path, { ...init, headers, signal: controller.signal });
  } catch {
    throw new CompanionError('not-running');
  } finally {
    clearTimeout(timer);
  }
  if (response.status === 401) {
    saveToken(null);
    throw new CompanionError('not-paired');
  }
  if (!response.ok) throw new Error(`Companion answered ${response.status}`);
  return (await response.json()) as T;
}

// ---------- Public API ----------

/** Checks that the app is running and that both sides speak a compatible API. */
export async function checkCompanion(): Promise<Hello> {
  const hello = await request<Hello>('/v1/hello');
  if (hello.apiVersion < API_VERSION) throw new CompanionError('companion-outdated');
  if (API_VERSION < hello.minClientApi) throw new CompanionError('addin-outdated');
  if (!loadToken()) throw new CompanionError('not-paired');
  return hello;
}

/**
 * Asks the app for permission to control it. The app shows an "Allow" prompt;
 * resolves true once the teacher clicks Allow, false if they decline.
 */
export async function pair(): Promise<boolean> {
  const { requestId } = await request<PairStart>('/v1/pair', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ clientName: 'PowerPoint add-in' }),
  });
  // Poll for the teacher's answer for up to 2 minutes.
  for (let i = 0; i < 120; i++) {
    await new Promise((r) => setTimeout(r, 1000));
    const poll = await request<PairPoll>(`/v1/pair/${encodeURIComponent(requestId)}`);
    if (poll.state === 'approved' && poll.token) {
      saveToken(poll.token);
      return true;
    }
    if (poll.state !== 'pending') return false;
  }
  return false;
}

export function getStatus(): Promise<Status> {
  return request<Status>('/v1/status');
}

export function sendCommand(command: Command): Promise<Status> {
  return request<Status>(`/v1/broadcast/${command}`, { method: 'POST' });
}

/**
 * Opens a live status feed. Calls `onStatus` on every change and `onClose`
 * when the connection drops (the caller decides whether to reconnect).
 */
export function subscribe(onStatus: (s: Status) => void, onClose: () => void): () => void {
  const token = loadToken();
  const ws = new WebSocket(CONTROL_BASE.replace('http', 'ws') + '/v1/events');
  ws.onopen = () => ws.send(JSON.stringify({ type: 'auth', token }));
  ws.onmessage = (event) => {
    try {
      const msg = JSON.parse(String(event.data)) as ServerMessage;
      if (msg.type === 'status') onStatus(msg.status);
    } catch {
      /* ignore malformed messages */
    }
  };
  ws.onclose = onClose;
  return () => {
    ws.onclose = null;
    ws.close();
  };
}
