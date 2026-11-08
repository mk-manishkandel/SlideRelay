// "Allow PowerPoint to use SlideRelay?" prompt shown during pairing.
import '@sliderelay/shared/theme.css';
import './app.css';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';

const id = new URLSearchParams(location.search).get('id') ?? '';
const $ = (elId: string) => document.getElementById(elId) as HTMLElement;

async function answer(allow: boolean): Promise<void> {
  try {
    await invoke('answer_pairing', { id, allow });
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
$('allow').focus();
