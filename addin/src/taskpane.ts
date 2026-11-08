/**
 * The SlideRelay task pane inside PowerPoint.
 *
 * It is only a remote control: all the real work (capturing the slideshow,
 * serving students) happens in the companion app. This file just shows the
 * app's status and sends button clicks to it.
 */
import '@sliderelay/shared/theme.css';
import './taskpane.css';
import type { Command, Status } from '@sliderelay/shared/protocol';
import {
  CompanionError,
  checkCompanion,
  getStatus,
  pair,
  sendCommand,
  subscribe,
  type ConnectionProblem,
} from './companion';

type View = 'checking' | ConnectionProblem | 'idle' | 'broadcasting';

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

// ---------- View switching ----------

let currentView: View = 'checking';

function showView(view: View): void {
  if (view === currentView) return;
  currentView = view;
  document.querySelectorAll<HTMLElement>('[data-view]').forEach((el) => {
    const visible = el.dataset.view === view;
    el.hidden = !visible;
    if (visible) el.classList.add('sr-fade-in');
  });
}

function showError(message: string | null): void {
  const el = $('error');
  el.textContent = message ?? '';
  el.hidden = !message;
}

// ---------- Rendering the live status ----------

function render(status: Status): void {
  const broadcasting = status.state !== 'idle';
  showView(broadcasting ? 'broadcasting' : 'idle');
  $('live-pill').hidden = status.state !== 'live';
  if (!broadcasting) return;

  let line: string;
  if (status.state === 'waiting') line = 'Start your slideshow to go live.';
  else if (status.blanked) line = 'Blanked — students see a black screen.';
  else if (status.paused) line = 'Paused — students see a frozen slide.';
  else line = 'Live — students see your slideshow.';
  $('status-line').textContent = line;
  $('status-line').dataset.tone = status.blanked || status.paused ? 'warn' : 'normal';

  $('viewer-count').textContent = String(status.viewers);

  const problem = $('problem');
  problem.textContent = status.problem ?? '';
  problem.hidden = !status.problem;

  $('pause-btn').textContent = status.paused ? 'Resume' : 'Pause';
  $('pause-btn').dataset.command = status.paused ? 'resume' : 'pause';
  $('blank-btn').textContent = status.blanked ? 'Show slides' : 'Blank screen';
  $('blank-btn').dataset.command = status.blanked ? 'unblank' : 'blank';
}

// ---------- Connecting ----------

let unsubscribe: (() => void) | null = null;
let retryTimer: number | undefined;

/** Tries to reach the app; on failure shows the right message and retries. */
async function connect(): Promise<void> {
  clearTimeout(retryTimer);
  try {
    await checkCompanion();
    render(await getStatus());
    unsubscribe?.();
    unsubscribe = subscribe(render, () => {
      // The app quit or crashed: go back to checking.
      unsubscribe = null;
      scheduleRetry();
    });
  } catch (err) {
    if (err instanceof CompanionError) {
      $('live-pill').hidden = true;
      showView(err.problem);
      // Keep retrying quietly, except while waiting for the teacher to pair.
      if (err.problem !== 'not-paired') scheduleRetry();
    } else {
      showError('Something went wrong talking to the SlideRelay app. Retrying…');
      scheduleRetry();
    }
  }
}

function scheduleRetry(): void {
  clearTimeout(retryTimer);
  retryTimer = window.setTimeout(connect, 2000);
}

async function run(command: Command): Promise<void> {
  showError(null);
  try {
    render(await sendCommand(command));
  } catch (err) {
    if (err instanceof CompanionError) {
      showView(err.problem);
      scheduleRetry();
    } else {
      showError("That didn't work. Please try again.");
    }
  }
}

// ---------- Wiring up buttons ----------

function wire(): void {
  $('pair-btn').addEventListener('click', async () => {
    const button = $<HTMLButtonElement>('pair-btn');
    button.disabled = true;
    $('pair-hint').hidden = false;
    showError(null);
    try {
      const result = await pair();
      if (result === 'denied') showError('Connection was not allowed. Click Connect to try again.');
      if (result === 'expired') showError('That took too long. Click Connect to try again.');
    } catch {
      showError('Could not reach the SlideRelay app. Make sure it is open.');
    } finally {
      button.disabled = false;
      $('pair-hint').hidden = true;
      connect();
    }
  });

  $('start-btn').addEventListener('click', () => run('start'));
  for (const id of ['pause-btn', 'blank-btn']) {
    $(id).addEventListener('click', (e) => {
      const command = (e.currentTarget as HTMLElement).dataset.command as Command;
      run(command);
    });
  }
  $('qr-btn').addEventListener('click', () => run('show-qr'));

  // Stop asks for a quick confirmation so a stray click doesn't end class.
  $('stop-btn').addEventListener('click', () => {
    $('stop-confirm').hidden = false;
    $('stop-cancel').focus();
  });
  $('stop-cancel').addEventListener('click', () => ($('stop-confirm').hidden = true));
  $('stop-yes').addEventListener('click', () => {
    $('stop-confirm').hidden = true;
    run('stop');
  });
}

// Office.onReady also fires when the page is opened in a normal browser
// (handy for development), so the pane works either way.
Office.onReady(() => {
  // On a Mac the add-in needs the app's one-time secure-connection step;
  // until it's done the app looks "not running", so point the way.
  $('mac-hint').hidden = Office.context?.diagnostics?.platform !== Office.PlatformType.Mac;
  wire();
  connect();
});
