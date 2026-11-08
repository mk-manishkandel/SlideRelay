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
  $('viewer-count').textContent = String(status.viewers);
  $('link').textContent = status.url ?? '';
}

async function command(name: string): Promise<void> {
  try {
    render(await invoke<Status>('run_command', { command: name }));
  } catch (err) {
    console.error(err);
  }
}

$('start-btn').addEventListener('click', () => command('start'));
$('stop-btn').addEventListener('click', () => command('stop'));

listen<Status>('status', (e) => render(e.payload));
invoke<Status>('get_status').then(render);
invoke<string>('app_version').then((v) => ($('version').textContent = v));
