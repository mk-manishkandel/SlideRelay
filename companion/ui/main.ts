// Main companion window: shows whether a broadcast is running.
import '@sliderelay/shared/theme.css';
import './app.css';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { Status } from '@sliderelay/shared/protocol';

const $ = (id: string) => document.getElementById(id) as HTMLElement;

function render(status: Status): void {
  const broadcasting = status.state !== 'idle';
  $('idle').hidden = broadcasting;
  $('broadcasting').hidden = !broadcasting;
  $('live-pill').hidden = status.state !== 'live';
  if (!broadcasting) return;

  $('status-line').textContent =
    status.state === 'waiting'
      ? 'Start your slideshow to go live'
      : status.blanked
        ? 'Blanked'
        : status.paused
          ? 'Paused'
          : 'Live';
  $('viewers').textContent =
    status.viewers === 1 ? '1 viewer connected' : `${status.viewers} viewers connected`;
  $('link').textContent = status.url ? status.url.replace(/^https?:\/\//, '') : '';
  $('qr-btn').hidden = !status.url;

  $('problem-card').hidden = !status.problem;
  $('problem-text').textContent = status.problem ?? '';
  // The one problem the app can fix for the presenter: macOS Screen Recording.
  $('fix-btn').hidden = !status.problem?.includes('Screen Recording');
}

async function command(name: string): Promise<void> {
  try {
    render(await invoke<Status>('run_command', { command: name }));
  } catch (err) {
    console.error(err);
  }
}

// ---------- One-time connection setup (macOS) ----------

async function refreshSetupCard(): Promise<void> {
  $('setup-card').hidden = !(await invoke<boolean>('connection_setup_needed'));
}

$('setup-btn').addEventListener('click', async () => {
  const button = $('setup-btn') as HTMLButtonElement;
  button.disabled = true;
  $('setup-error').hidden = true;
  try {
    await invoke('setup_connection');
    await refreshSetupCard();
  } catch (err) {
    $('setup-error').textContent = String(err);
    $('setup-error').hidden = false;
  } finally {
    button.disabled = false;
  }
});

refreshSetupCard();

$('start-btn').addEventListener('click', () => command('start'));
$('stop-btn').addEventListener('click', () => command('stop'));
$('qr-btn').addEventListener('click', () => command('show-qr'));
$('fix-btn').addEventListener('click', () => invoke('open_screen_permission_settings'));

listen<Status>('status', (e) => render(e.payload));
invoke<Status>('get_status').then(render);
invoke<string>('app_version').then((v) => ($('version').textContent = v));
