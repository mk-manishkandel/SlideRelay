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
import { reloadIfOutdated } from './self-update';
import { isStudentLink, qrSvg, shortLink } from './qr';

declare const __BUILD_ID__: string;

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
  renderShare(broadcasting ? status.url : null);
  if (!broadcasting) return;

  let line: string;
  if (status.state === 'waiting') line = 'Start your slideshow to go live.';
  else if (status.blanked) line = 'Blanked — students see a black screen.';
  else if (status.paused) line = 'Paused — students see a frozen slide.';
  else line = 'Live — students see your slideshow.';
  $('status-line').textContent = line;
  $('status-line').dataset.tone = status.blanked || status.paused ? 'warn' : 'normal';

  $('viewers').textContent =
    status.viewers === 1 ? '1 student connected' : `${status.viewers} students connected`;

  const problem = $('problem');
  problem.textContent = status.problem ?? '';
  problem.hidden = !status.problem;

  $('pause-btn').textContent = status.paused ? 'Resume' : 'Pause';
  $('pause-btn').dataset.command = status.paused ? 'resume' : 'pause';
  $('blank-btn').textContent = status.blanked ? 'Show slides' : 'Blank screen';
  $('blank-btn').dataset.command = status.blanked ? 'unblank' : 'blank';
}

// ---------- Student link and QR code ----------

let shownUrl: string | null = null;
let qrDialog: Office.Dialog | null = null;

/** Draws the link and QR in the panel; only redraws when the link changes. */
function renderShare(url: string | null): void {
  const link = isStudentLink(url) ? url : null;
  if (link === shownUrl) return;
  shownUrl = link;
  $('share').hidden = !link;
  $('share-qr').replaceChildren(...(link ? [qrSvg(link)] : []));
  $('share-link').textContent = link ? shortLink(link) : '';
  // The link changed (new network) or the broadcast stopped: the big QR is stale.
  closeQrDialog();
}

function closeQrDialog(): void {
  qrDialog?.close();
  qrDialog = null;
}

/**
 * Opens the big QR code as a PowerPoint dialog window. It belongs to
 * PowerPoint, not the SlideRelay app, so closing it can't quit the app.
 */
function showQrDialog(): void {
  if (!shownUrl) return;
  closeQrDialog();
  const page = new URL('qr.html', location.href);
  page.searchParams.set('url', shownUrl);
  Office.context.ui.displayDialogAsync(
    page.href,
    { width: 80, height: 85, displayInIframe: false },
    (result) => {
      if (result.status !== Office.AsyncResultStatus.Succeeded) {
        showError('Could not open the QR window. Students can scan the code above.');
        return;
      }
      const dialog = result.value;
      qrDialog = dialog;
      dialog.addEventHandler(Office.EventType.DialogMessageReceived, () => closeQrDialog());
      dialog.addEventHandler(Office.EventType.DialogEventReceived, () => {
        if (qrDialog === dialog) qrDialog = null;
      });
    },
  );
}

async function copyLink(): Promise<void> {
  if (!shownUrl) return;
  const button = $('copy-btn');
  try {
    await navigator.clipboard.writeText(shownUrl);
  } catch {
    // Older Office webviews: copy through a hidden text field.
    const field = document.createElement('textarea');
    field.value = shownUrl;
    document.body.append(field);
    field.select();
    document.execCommand('copy');
    field.remove();
  }
  button.textContent = 'Copied';
  setTimeout(() => (button.textContent = 'Copy link'), 1500);
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
      renderShare(null);
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

  // The button is a target=_blank link to sliderelay://open: Office hands
  // new-window links to the operating system, which launches the app that
  // registered the sliderelay:// scheme. (A plain link inside the panel is
  // ignored.) "Didn't open?" falls back to a page in the teacher's browser.
  // Meanwhile keep retrying so the panel connects as soon as the app is up.
  $('open-app-btn').addEventListener('click', () => scheduleRetry());

  $('start-btn').addEventListener('click', () => run('start'));
  for (const id of ['pause-btn', 'blank-btn']) {
    $(id).addEventListener('click', (e) => {
      const command = (e.currentTarget as HTMLElement).dataset.command as Command;
      run(command);
    });
  }
  $('qr-btn').addEventListener('click', showQrDialog);
  $('copy-btn').addEventListener('click', copyLink);

  // Stop asks for a quick confirmation so a stray click doesn't end class.
  // The confirmation takes the Stop button's place, so only one is ever shown.
  const showStopConfirm = (show: boolean) => {
    $('stop-btn').hidden = show;
    $('stop-confirm').hidden = !show;
  };
  $('stop-btn').addEventListener('click', () => {
    showStopConfirm(true);
    $('stop-cancel').focus();
  });
  $('stop-cancel').addEventListener('click', () => {
    showStopConfirm(false);
    $('stop-btn').focus();
  });
  $('stop-yes').addEventListener('click', () => {
    showStopConfirm(false);
    run('stop');
  });
}

// Office.onReady also fires when the page is opened in a normal browser
// (handy for development), so the pane works either way.
reloadIfOutdated();
$('build-id').textContent = __BUILD_ID__.slice(0, 7);

Office.onReady(() => {
  // On a Mac the add-in needs the app's one-time secure-connection step;
  // until it's done the app looks "not running", so point the way.
  $('mac-hint').hidden = Office.context?.diagnostics?.platform !== Office.PlatformType.Mac;
  wire();
  connect();
});
