// Floating toolbar shown while broadcasting. It never takes the keyboard,
// so PowerPoint keeps getting the arrow keys.
import '@sliderelay/shared/theme.css';
import './app.css';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { icons } from '@sliderelay/shared/icons';
import type { Status } from '@sliderelay/shared/protocol';

const $ = (id: string) => document.getElementById(id) as HTMLElement;
const isMac = navigator.userAgent.includes('Mac');
const key = (letter: string) => (isMac ? `⌃⌥${letter}` : `Ctrl+Alt+${letter}`);

let status: Status | null = null;

/** Sets a button's icon, label and tooltip (static icon markup only). */
function setButton(id: string, icon: keyof typeof icons, label: string, tip: string): void {
  const button = $(id);
  (button.querySelector('.tb-icon') as HTMLElement).innerHTML = icons[icon];
  (button.querySelector('.tb-label') as HTMLElement).textContent = label;
  button.title = tip;
  button.setAttribute('aria-label', tip);
}

function render(s: Status): void {
  status = s;
  let state = 'Live';
  let tone = 'live';
  if (s.blanked) [state, tone] = ['Blank', 'warn'];
  else if (s.paused) [state, tone] = ['Paused', 'warn'];
  else if (s.state === 'waiting') [state, tone] = ['Waiting', 'idle'];
  $('state').textContent = state;
  $('status-pill').dataset.tone = tone;
  $('status-pill').title =
    s.state === 'waiting'
      ? 'Start your slideshow and viewers see it live'
      : s.blanked
        ? 'Viewers see "Screen hidden by the presenter"'
        : s.paused
          ? 'Viewers keep seeing the slide from when you paused'
          : 'Viewers see your slideshow live';
  $('pause').setAttribute('aria-pressed', String(s.paused));
  $('blank').setAttribute('aria-pressed', String(s.blanked));
  $('viewers').textContent = String(s.viewers);
  $('people').parentElement!.title =
    s.viewers === 1 ? '1 viewer watching' : `${s.viewers} viewers watching`;

  if (s.paused) setButton('pause', 'play', 'Resume', `Resume (${key('P')})`);
  else setButton('pause', 'pause', 'Pause', `Pause: viewers keep seeing this slide (${key('P')})`);
  if (s.blanked) setButton('blank', 'unblank', 'Show', `Show slides again (${key('B')})`);
  else setButton('blank', 'blank', 'Blank', `Blank: hide the slide from viewers (${key('B')})`);
}

async function send(command: string): Promise<void> {
  try {
    render(await invoke<Status>('toolbar_command', { command }));
  } catch {
    /* the status event will correct the display */
  }
}

function confirmStop(show: boolean): void {
  $('actions').hidden = show;
  $('confirm').hidden = !show;
}

$('people').innerHTML = icons.people;
$('grip').innerHTML = icons.grip;
setButton('qr', 'qr', 'QR', `Show the QR code on the projector (${key('Q')})`);
setButton('stop', 'stop', 'Stop', 'Stop the broadcast');

$('pause').addEventListener('click', () => send(status?.paused ? 'resume' : 'pause'));
$('blank').addEventListener('click', () => send(status?.blanked ? 'unblank' : 'blank'));
$('qr').addEventListener('click', () => send('toggle-qr'));
$('stop').addEventListener('click', () => confirmStop(true));
$('stop-no').addEventListener('click', () => {
  confirmStop(false);
  send('refocus');
});
$('stop-yes').addEventListener('click', () => {
  confirmStop(false);
  send('stop');
});

// ---------- Hover and press highlights ----------
// The toolbar never becomes the active window, so the browser doesn't always
// hear the mouse leave, and CSS :hover / :active can get stuck after a click.
// We track them ourselves and always clear them after a click.
const buttons = Array.from(document.querySelectorAll<HTMLButtonElement>('.tb-btn'));
const clearHighlights = () => buttons.forEach((b) => b.classList.remove('is-hover', 'is-pressed'));

buttons.forEach((b) => {
  b.addEventListener('pointerenter', () => b.classList.add('is-hover'));
  b.addEventListener('pointerleave', () => b.classList.remove('is-hover', 'is-pressed'));
  b.addEventListener('pointerdown', () => b.classList.add('is-pressed'));
  b.addEventListener('pointerup', () => b.classList.remove('is-pressed'));
  b.addEventListener('click', () => {
    clearHighlights();
    b.blur();
  });
});
document.documentElement.addEventListener('pointerleave', clearHighlights);
window.addEventListener('blur', clearHighlights);
// If the "leave" never arrives, the next move elsewhere clears it.
document.addEventListener('pointermove', (e) => {
  const over = (e.target as Element).closest('.tb-btn');
  buttons.forEach((b) => b !== over && b.classList.remove('is-hover', 'is-pressed'));
});

// Dragging (anywhere that isn't a button): from now on the toolbar stays
// where the presenter puts it.
$('toolbar').addEventListener('mousedown', (e) => {
  if (e.button === 0 && !(e.target as Element).closest('button')) {
    clearHighlights();
    invoke('toolbar_moved').catch(() => {});
  }
});

listen<Status>('status', (e) => render(e.payload));
invoke<Status>('get_status').then(render);
