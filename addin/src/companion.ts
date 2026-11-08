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
  CONTROL_BASES,
  type Command,
  type Hello,
  type PairPoll,
  type PairStart,
  type ApiErrorBody,
  type ServerMessage,
  type SettingsView,
  type Status,
  type TeacherSettings,
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

/** The app refused a request; `reason` is a plain-words message, if any. */
export class ApiError extends Error {
  constructor(
    public readonly status: number,
    public readonly reason: string | null,
  ) {
    super(reason ?? `Companion answered ${status}`);
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

/** The address that worked last time (null = try them all). */
let workingBase: string | null = null;

async function fetchWithTimeout(url: string, init: RequestInit, timeoutMs: number) {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  try {
    return await fetch(url, { ...init, signal: controller.signal });
  } finally {
    clearTimeout(timer);
  }
}

async function request<T>(path: string, init: RequestInit = {}, timeoutMs = 3000): Promise<T> {
  const headers = new Headers(init.headers);
  const token = loadToken();
  if (token) headers.set('Authorization', `Bearer ${token}`);

  // Try the address that worked before first, then the others.
  const bases = workingBase
    ? [workingBase, ...CONTROL_BASES.filter((b) => b !== workingBase)]
    : [...CONTROL_BASES];
  let response: Response | null = null;
  for (const base of bases) {
    try {
      response = await fetchWithTimeout(base + path, { ...init, headers }, timeoutMs);
      workingBase = base;
      break;
    } catch {
      // Blocked or nothing listening: try the next address.
    }
  }
  if (!response) {
    workingBase = null;
    throw new CompanionError('not-running');
  }
  if (response.status === 401) {
    saveToken(null);
    throw new CompanionError('not-paired');
  }
  if (!response.ok) {
    let reason: string | null = null;
    try {
      reason = ((await response.json()) as ApiErrorBody).error ?? null;
    } catch {
      /* no JSON body */
    }
    throw new ApiError(response.status, reason);
  }
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
 * resolves with the teacher's answer ('approved', 'denied' or 'expired').
 */
export async function pair(): Promise<PairPoll['state']> {
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
      return 'approved';
    }
    if (poll.state !== 'pending') return poll.state;
  }
  return 'expired';
}

export function getStatus(): Promise<Status> {
  return request<Status>('/v1/status');
}

export function sendCommand(command: Command): Promise<Status> {
  return request<Status>(`/v1/broadcast/${command}`, { method: 'POST' });
}

/** Asks the app to open the right system page to fix a problem. */
export async function requestFix(fix: string): Promise<void> {
  const response = await fetch(`${workingBase ?? CONTROL_BASES[0]}/v1/fix/${fix}`, {
    method: 'POST',
    headers: { Authorization: `Bearer ${loadToken() ?? ''}` },
  });
  if (!response.ok) throw new ApiError(response.status, null);
}

export function getSettings(): Promise<SettingsView> {
  return request<SettingsView>('/v1/settings');
}

export function saveSettings(settings: TeacherSettings): Promise<SettingsView> {
  return request<SettingsView>('/v1/settings', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(settings),
  });
}

/**
 * Opens a live status feed. Calls `onStatus` on every change and `onClose`
 * when the connection drops (the caller decides whether to reconnect).
 */
export function subscribe(onStatus: (s: Status) => void, onClose: () => void): () => void {
  const token = loadToken();
  const base = workingBase ?? CONTROL_BASES[0];
  const ws = new WebSocket(base.replace(/^http/, 'ws') + '/v1/events');
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
