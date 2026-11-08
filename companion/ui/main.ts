// Main SlideRelay window: the setup guide (first launch, or from the menu)
// and the broadcast status. Day to day, presenters use the PowerPoint panel.
import '@sliderelay/shared/theme.css';
import './app.css';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { Status } from '@sliderelay/shared/protocol';

type UpdateCheck =
  | { state: 'unavailable' }
  | { state: 'up-to-date' }
  | { state: 'ready'; version: string }
  | { state: 'failed'; message: string };

interface SetupState {
  platform: 'macos' | 'windows';
  addinInstalled: boolean;
  connectionNeeded: boolean;
  screenPermissionNeeded: boolean;
  setupDone: boolean;
}

const $ = (id: string) => document.getElementById(id) as HTMLElement;

// ---------- Broadcast status ----------

let broadcasting = false;
let setupComplete = false;

/** While broadcasting with setup finished, the window shows only the status. */
function updateSetupVisibility(): void {
  $('setup').hidden = broadcasting && setupComplete;
}

function render(status: Status): void {
  broadcasting = status.state !== 'idle';
  updateSetupVisibility();
  $('broadcasting').hidden = !broadcasting;
  $('live-pill').hidden = status.state !== 'live' || status.paused || status.blanked;
  if (!broadcasting) return;

  const [label, tone, hint] =
    status.state === 'waiting'
      ? ['Waiting for slideshow', 'idle', 'Start your slideshow to go live.']
      : status.blanked
        ? ['Blanked', 'warn', 'Viewers see "Screen hidden by the presenter".']
        : status.paused
          ? ['Paused', 'warn', 'Viewers keep seeing the slide from when you paused.']
          : ['Live', 'live', ''];
  $('status-line').textContent = label;
  $('bc-state').dataset.tone = tone;
  $('bc-hint').textContent = hint;
  $('bc-hint').hidden = !hint;
  $('viewers').textContent = status.viewers === 1 ? '1 viewer' : `${status.viewers} viewers`;
  $('link').textContent = status.url ? status.url.replace(/^https?:\/\//, '') : '';
  $('problem-card').hidden = !status.problem;
  $('problem-text').textContent = status.problem ?? '';
  $('fix-btn').hidden = status.fix !== 'screen-permission';
}

// ---------- Setup guide ----------

function mark(stepId: string, done: boolean): void {
  $(stepId).classList.toggle('is-done', done);
}

let stepsOpen = false;

async function refreshSetup(): Promise<void> {
  const s = await invoke<SetupState>('setup_state');
  document
    .querySelectorAll<HTMLElement>('[data-mac-only]')
    .forEach((el) => (el.hidden = s.platform !== 'macos'));
  if (s.platform === 'windows') {
    $('setup-intro').textContent =
      'A few one-time steps. SlideRelay then lives in the system tray, next to the clock.';
  }
  mark('step-addin', s.addinInstalled);
  mark('step-connect', !s.connectionNeeded);
  mark('step-screen', !s.screenPermissionNeeded);
  const allDone =
    s.addinInstalled &&
    (s.platform !== 'macos' || (!s.connectionNeeded && !s.screenPermissionNeeded));
  setupComplete = allDone;
  updateSetupVisibility();
  $('setup-title').textContent = allDone ? 'SlideRelay is ready' : 'Set up SlideRelay';
  // Done: fold the steps into one line (they can be reopened).
  $('setup-intro').hidden = allDone;
  $('setup-ready').hidden = !allDone;
  $('steps').hidden = allDone && !stepsOpen;
  $('later-note').hidden = allDone;
  $('show-steps').hidden = stepsOpen;
  if (s.platform === 'windows') {
    $('later-note').innerHTML =
      'You can close this window and finish later. SlideRelay stays in the system tray; ' +
      'right-click its icon and choose <strong>Setup and status</strong> to come back.';
  }
}

async function runStep(button: string, error: string, command: string): Promise<void> {
  const btn = $(button) as HTMLButtonElement;
  btn.disabled = true;
  $(error).hidden = true;
  try {
    await invoke(command);
  } catch (err) {
    $(error).textContent = String(err);
    $(error).hidden = false;
  } finally {
    btn.disabled = false;
    await refreshSetup();
  }
}

$('addin-btn').addEventListener('click', () =>
  runStep('addin-btn', 'addin-error', 'install_addin'),
);
$('setup-btn').addEventListener('click', () =>
  runStep('setup-btn', 'setup-error', 'setup_connection'),
);
$('screen-btn').addEventListener('click', () => invoke('open_screen_permission_settings'));
$('fix-btn').addEventListener('click', () => invoke('open_screen_permission_settings'));
$('help-btn').addEventListener('click', () => invoke('open_help'));
$('show-steps').addEventListener('click', () => {
  stepsOpen = true;
  refreshSetup();
});

// ---------- Updates ----------

function showUpdate(result: UpdateCheck): void {
  const status = $('update-status');
  $('install-btn').hidden = result.state !== 'ready';
  status.textContent =
    result.state === 'up-to-date'
      ? 'You have the newest version.'
      : result.state === 'ready'
        ? `Version ${result.version} is ready. It also installs by itself when SlideRelay quits.`
        : result.state === 'unavailable'
          ? "This test build doesn't update itself. Download SlideRelay from the project page."
          : result.message;
}

$('update-btn').addEventListener('click', async () => {
  const btn = $('update-btn') as HTMLButtonElement;
  btn.disabled = true;
  $('install-btn').hidden = true;
  $('update-status').textContent = 'Checking…';
  try {
    showUpdate(await invoke<UpdateCheck>('check_for_updates'));
  } catch (err) {
    showUpdate({ state: 'failed', message: String(err) });
  } finally {
    btn.disabled = false;
  }
});

$('install-btn').addEventListener('click', async () => {
  $('update-status').textContent = 'Installing… SlideRelay reopens in a moment.';
  try {
    await invoke('install_update_now');
  } catch (err) {
    $('update-status').textContent = String(err);
  }
});
$('stop-btn').addEventListener('click', () => invoke('run_command', { command: 'stop' }));

// Permission changes happen in System Settings: check again when we're back.
window.addEventListener('focus', refreshSetup);
setInterval(() => {
  if (document.visibilityState === 'visible') refreshSetup();
}, 3000);

listen<Status>('status', (e) => render(e.payload));
invoke<Status>('get_status').then(render);
invoke<string>('app_version').then((v) => ($('version').textContent = v));
refreshSetup();
