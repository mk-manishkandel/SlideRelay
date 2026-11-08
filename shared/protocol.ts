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

/** POST /v1/pair → starts a pairing request the presenter must approve. */
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
  /** The link viewers open, e.g. "http://192.168.1.20:8080/K7PQ". */
  url: string | null;
  /** Problem the presenter should know about, in plain words (or null). */
  problem: string | null;
  /** A fix the panel can offer for the problem: POST /v1/fix/{fix}. */
  fix?: 'screen-permission' | null;
}

export type Quality = 'low' | 'balanced' | 'high';
export type ToolbarPosition = 'top-center' | 'bottom-center' | 'top-right' | 'bottom-right';

/** The presenter's settings, saved by the app. */
export interface PresenterSettings {
  /** Port viewers connect to; 0 = automatic. */
  port: number;
  quality: Quality;
  /** Maximum picture updates per second sent to viewers (1–30). */
  maxFps: number;
  /** Network address to put in the viewer link; null = automatic. */
  lanIp: string | null;
  /** PIN viewers must enter (4–8 digits), or null for none. */
  pin: string | null;
  toolbarPosition: ToolbarPosition;
  launchAtLogin: boolean;
  /** Read-only here: set by the app's setup wizard. */
  setupDone: boolean;
}

/** A network adapter viewers might reach the app on. */
export interface Adapter {
  name: string;
  ip: string;
}

/** GET /v1/settings, and the answer to POST /v1/settings. */
export interface SettingsView {
  settings: PresenterSettings;
  adapters: Adapter[];
  /** The port viewers are using right now. */
  portInUse: number | null;
}

/** Error body for a refused POST /v1/settings (400). */
export interface ApiErrorBody {
  /** Why, in plain words for the presenter. */
  error: string;
}

/** Commands: POST /v1/broadcast/{command} */
export type Command = 'start' | 'stop' | 'pause' | 'resume' | 'blank' | 'unblank' | 'show-qr';

/** Messages on WS /v1/events */
export type ClientMessage = { type: 'auth'; token: string };
export type ServerMessage = { type: 'status'; status: Status } | { type: 'error'; message: string };
