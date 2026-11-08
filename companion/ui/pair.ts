// "Allow PowerPoint to use SlideRelay?" prompt shown during pairing.
import '@sliderelay/shared/theme.css';
import './app.css';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';

/** Matches REQUEST_TTL in src-tauri/src/pairing.rs. */
const REQUEST_TTL_MS = 120_000;

const $ = (elId: string) => document.getElementById(elId) as HTMLElement;

let requestId = new URLSearchParams(location.search).get('id') ?? '';
let expiryTimer: number | undefined;

/** Shows the question for a (new) request and closes the window if it expires. */
function showRequest(id: string): void {
  requestId = id;
  $('title').textContent = 'Allow PowerPoint to use SlideRelay?';
  $('body').innerHTML =
    'Only click Allow if you just clicked <strong>Connect</strong> in PowerPoint.';
  $('actions').hidden = false;
  $('allow').focus();
  clearTimeout(expiryTimer);
  expiryTimer = window.setTimeout(() => getCurrentWindow().close(), REQUEST_TTL_MS);
}

async function answer(allow: boolean): Promise<void> {
  clearTimeout(expiryTimer);
  try {
    await invoke('answer_pairing', { id: requestId, allow });
    if (allow) {
      $('title').textContent = 'Connected';
      $('body').textContent = 'You can go back to PowerPoint now.';
      $('actions').hidden = true;
      setTimeout(() => getCurrentWindow().close(), 1500);
    } else {
      getCurrentWindow().close();
    }
  } catch (err) {
    $('body').textContent = String(err);
    $('actions').hidden = true;
  }
}

$('allow').addEventListener('click', () => answer(true));
$('deny').addEventListener('click', () => answer(false));

// The app sends a new request id here if Connect is clicked again while open.
listen<string>('pair-request', (e) => showRequest(e.payload));
showRequest(requestId);
