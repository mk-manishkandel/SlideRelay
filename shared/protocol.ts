/**
 * The versioned API between the PowerPoint add-in and the SlideRelay
 * companion app. The Rust side mirrors these types in
 * `companion/src-tauri/src/control_api.rs` — keep both in sync.
 *
 * Versioning rule: add fields freely (old clients ignore them). Removing or
 * changing the meaning of something bumps API_VERSION, and the companion
 * reports the oldest client API it still accepts in `minClientApi`.
 */

/** API version spoken by this build of the add-in. */
export const API_VERSION = 1;

/** The companion listens only on the loopback interface, on this port. */
export const CONTROL_PORT = 47615;
/** macOS only: the same API over HTTPS with a locally trusted certificate. */
export const CONTROL_TLS_PORT = 47616;

/**
 * Addresses to try, in order. Windows (WebView2) accepts plain http to
 * 127.0.0.1; PowerPoint for Mac blocks it, so it uses the HTTPS one.
 */
export const CONTROL_BASES = [
  `http://127.0.0.1:${CONTROL_PORT}`,
  `https://localhost:${CONTROL_TLS_PORT}`,
] as const;

/** GET /v1/hello — public, no token needed. */
export interface Hello {
  app: 'SlideRelay';
  /** Companion app version, e.g. "1.2.0". */
  version: string;
  /** API version the companion speaks. */
  apiVersion: number;
  /** Oldest add-in API version the companion still accepts. */
  minClientApi: number;
}

/** POST /v1/pair → starts a pairing request the teacher must approve. */
export interface PairStart {
  requestId: string;
}

/** GET /v1/pair/{requestId} */
export interface PairPoll {
  state: 'pending' | 'approved' | 'denied' | 'expired';
  /** Present once, when state === 'approved'. Store it; send as Bearer token. */
  token?: string;
}

export type BroadcastState =
  /** Not broadcasting. */
  | 'idle'
  /** Broadcasting, but no slideshow is running yet. */
  | 'waiting'
  /** Broadcasting the slideshow window. */
  | 'live';

/** GET /v1/status and the `status` event on the WebSocket. */
export interface Status {
  state: BroadcastState;
  paused: boolean;
  blanked: boolean;
  viewers: number;
  /** The link students open, e.g. "http://192.168.1.20:8080/K7PQ". */
  url: string | null;
  /** Problem the teacher should know about, in plain words (or null). */
  problem: string | null;
}

/** Commands: POST /v1/broadcast/{command} */
export type Command = 'start' | 'stop' | 'pause' | 'resume' | 'blank' | 'unblank' | 'show-qr';

/** Messages on WS /v1/events */
export type ClientMessage = { type: 'auth'; token: string };
export type ServerMessage = { type: 'status'; status: Status } | { type: 'error'; message: string };
