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

function render(status: Status): void {
  const broadcasting = status.state !== 'idle';
  $('broadcasting').hidden = !broadcasting;
  $('live-pill').hidden = status.state !== 'live';
  if (!broadcasting) return;

  $('status-line').textContent =
    status.state === 'waiting'
      ? 'Broadcasting: start your slideshow to go live'
      : status.blanked
        ? 'Blanked'
        : status.paused
          ? 'Paused'
          : 'Live';
  $('viewers').textContent =
    status.viewers === 1 ? '1 viewer connected' : `${status.viewers} viewers connected`;
  $('link').textContent = status.url ? status.url.replace(/^https?:\/\//, '') : '';
  $('problem-card').hidden = !status.problem;
  $('problem-text').textContent = status.problem ?? '';
  $('fix-btn').hidden = status.fix !== 'screen-permission';
}

// ---------- Setup guide ----------

function mark(stepId: string, done: boolean): void {
  $(stepId).classList.toggle('is-done', done);
}

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
  $('setup-title').textContent = allDone ? 'SlideRelay is ready' : 'Set up SlideRelay';
  $('done-btn').classList.toggle('sr-btn--primary', allDone);
  $('done-btn').textContent = allDone ? 'Done' : 'Finish later';
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
$('done-btn').addEventListener('click', () => invoke('finish_setup'));

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
