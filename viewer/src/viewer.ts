/**
 * Student viewer: connects to the teacher's SlideRelay app over the local
 * Wi-Fi and shows the live slideshow.
 *
 * Messages from the app:
 *   text   {"type":"state","mode":"waiting|live|paused|blank|ended"}
 *   text   {"type":"pin"} / {"type":"pin-wrong"}
 *   binary a JPEG of the current slide
 * The page only ever sends a PIN answer; viewers are read-only.
 */
import './viewer.css';

type Mode = 'waiting' | 'live' | 'paused' | 'blank' | 'ended';

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;
const slide = $<HTMLImageElement>('slide');
const message = $('message');
const pinForm = $<HTMLFormElement>('pin-form');
const pinInput = $<HTMLInputElement>('pin-input');
const fsButton = $<HTMLButtonElement>('fullscreen-btn');

/** The session code is the page path: http://192.168.1.20/K7PQ */
const code = decodeURIComponent(location.pathname.replace(/^\/+|\/+$/g, ''));

let socket: WebSocket | null = null;
let mode: Mode | null = null;
let ended = false;
let hasFrame = false;
let retryDelay = 1000;
let currentUrl: string | null = null;

// ---------- Messages ----------

function showMessage(title: string, detail = ''): void {
  $('message-title').textContent = title;
  $('message-detail').textContent = detail;
  message.hidden = false;
}

function hideMessage(): void {
  message.hidden = true;
}

function applyMode(next: Mode): void {
  mode = next;
  document.body.classList.toggle('blank', next === 'blank');
  $('paused-tag').hidden = next !== 'paused';

  switch (next) {
    case 'waiting':
      showMessage('Waiting for the presenter…', 'The slides will appear here automatically.');
      slide.hidden = true;
      hasFrame = false;
      break;
    case 'live':
    case 'paused':
      if (hasFrame) hideMessage();
      else showMessage('Waiting for the presenter…');
      break;
    case 'blank':
      hideMessage(); // plain black screen
      break;
    case 'ended':
      ended = true;
      slide.hidden = true;
      showMessage('The broadcast has ended', 'You can close this page.');
      break;
  }
}

function showFrame(data: Blob): void {
  const url = URL.createObjectURL(data);
  const previous = currentUrl;
  currentUrl = url;
  slide.onload = () => {
    if (previous) URL.revokeObjectURL(previous);
  };
  slide.src = url;
  slide.hidden = false;
  hasFrame = true;
  if (mode === 'live' || mode === 'paused') hideMessage();
}

// ---------- Connection ----------

function connect(): void {
  const proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
  const ws = new WebSocket(`${proto}//${location.host}/ws/${encodeURIComponent(code)}`);
  ws.binaryType = 'blob';
  socket = ws;

  ws.onopen = () => {
    retryDelay = 1000;
  };

  ws.onmessage = (event) => {
    if (event.data instanceof Blob) {
      showFrame(event.data);
      return;
    }
    let msg: { type?: string; mode?: Mode };
    try {
      msg = JSON.parse(String(event.data));
    } catch {
      return;
    }
    if (msg.type === 'state' && msg.mode) applyMode(msg.mode);
    else if (msg.type === 'pin') askPin(false);
    else if (msg.type === 'pin-wrong') askPin(true);
  };

  ws.onclose = () => {
    socket = null;
    if (ended) return;
    // Wi-Fi blip or the teacher's laptop slept: keep trying quietly.
    showMessage('Reconnecting…', 'Check that you are on the same Wi-Fi as your teacher.');
    setTimeout(connect, retryDelay);
    retryDelay = Math.min(retryDelay * 2, 5000);
  };
}

function askPin(wrong: boolean): void {
  hideMessage();
  pinForm.hidden = false;
  $('pin-error').hidden = !wrong;
  pinInput.value = '';
  pinInput.focus();
}

pinForm.addEventListener('submit', (e) => {
  e.preventDefault();
  socket?.send(JSON.stringify({ type: 'pin', pin: pinInput.value.trim() }));
  pinForm.hidden = true;
  showMessage('Joining…');
});

// ---------- Full screen button (appears on tap, then fades) ----------

let fadeTimer: number | undefined;
const canFullscreen =
  !!document.documentElement.requestFullscreen ||
  // Older Safari
  !!(document.documentElement as unknown as { webkitRequestFullscreen?: () => void })
    .webkitRequestFullscreen;

function showFsButton(): void {
  if (!canFullscreen || document.fullscreenElement) return;
  fsButton.hidden = false;
  fsButton.classList.remove('fading');
  clearTimeout(fadeTimer);
  fadeTimer = window.setTimeout(() => {
    fsButton.classList.add('fading');
    fadeTimer = window.setTimeout(() => (fsButton.hidden = true), 200);
  }, 3000);
}

document.addEventListener('click', (e) => {
  if (e.target !== fsButton && pinForm.hidden) showFsButton();
});

fsButton.addEventListener('click', async () => {
  const el = document.documentElement as HTMLElement & { webkitRequestFullscreen?: () => void };
  try {
    if (el.requestFullscreen) await el.requestFullscreen();
    else el.webkitRequestFullscreen?.();
    // Phones: prefer landscape for slides (ignored where unsupported).
    const orientation = screen.orientation as ScreenOrientation & {
      lock?: (o: string) => Promise<void>;
    };
    await orientation?.lock?.('landscape').catch(() => undefined);
  } catch {
    /* not allowed — nothing to do */
  }
  fsButton.hidden = true;
});

// ---------- Start ----------

if (!code) {
  showMessage('Almost there', 'Type the full link shown on your teacher’s screen.');
} else {
  showMessage('Connecting…');
  connect();
}
