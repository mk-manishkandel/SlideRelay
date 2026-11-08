/**
 * The SlideRelay task pane inside PowerPoint.
 *
 * It is only a remote control: all the real work (capturing the slideshow,
 * serving students) happens in the companion app. This file just shows the
 * app's status and sends button clicks to it.
 */
import '@sliderelay/shared/theme.css';
import './taskpane.css';
import type {
  Command,
  Quality,
  SettingsView,
  Status,
  TeacherSettings,
  ToolbarPosition,
} from '@sliderelay/shared/protocol';
import { icons } from '@sliderelay/shared/icons';
import {
  ApiError,
  CompanionError,
  checkCompanion,
  getSettings,
  getStatus,
  pair,
  requestFix,
  saveSettings,
  sendCommand,
  subscribe,
  type ConnectionProblem,
} from './companion';
import { reloadIfOutdated } from './self-update';
import { isStudentLink, qrSvg, shortLink } from './qr';

declare const __BUILD_ID__: string;

type View = 'checking' | ConnectionProblem | 'idle' | 'broadcasting' | 'settings';

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

// ---------- View switching ----------

let currentView: View = 'checking';
/** While Settings is open, status updates wait here instead of switching views. */
let settingsOpen = false;
let viewBehindSettings: View = 'idle';

function showView(view: View): void {
  const connected = view === 'idle' || view === 'broadcasting' || view === 'settings';
  if (settingsOpen && view !== 'settings') {
    if (connected) {
      viewBehindSettings = view;
      return;
    }
    // Lost the app: leave Settings and show what's wrong.
    settingsOpen = false;
  }
  $('settings-btn').hidden = !connected || view === 'settings';
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

let lastStatus: Status | null = null;

function render(status: Status): void {
  lastStatus = status;
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
  const fix = $('fix-btn');
  fix.hidden = !status.fix;
  if (status.fix) fix.dataset.fix = status.fix;

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

// ---------- Settings ----------

let settingsView: SettingsView | null = null;

function fillSettings(view: SettingsView): void {
  settingsView = view;
  const s = view.settings;
  $<HTMLSelectElement>('set-quality').value = s.quality;
  // Pick the closest offered speed (an older app may have saved another).
  const fps = [5, 10, 15, 30].reduce((a, b) =>
    Math.abs(b - s.maxFps) < Math.abs(a - s.maxFps) ? b : a,
  );
  $<HTMLSelectElement>('set-fps').value = String(fps);
  $<HTMLInputElement>('set-pin-on').checked = s.pin !== null;
  $<HTMLInputElement>('set-pin').value = s.pin ?? '';
  $('set-pin').hidden = s.pin === null;

  const network = $<HTMLSelectElement>('set-network');
  const options = [new Option('Automatic (recommended)', '')];
  for (const a of view.adapters) options.push(new Option(`${a.name} (${a.ip})`, a.ip));
  if (s.lanIp && !view.adapters.some((a) => a.ip === s.lanIp)) {
    options.push(new Option(`${s.lanIp} (not connected now)`, s.lanIp));
  }
  network.replaceChildren(...options);
  network.value = s.lanIp ?? '';

  const port = $<HTMLInputElement>('set-port');
  port.value = s.port === 0 ? '' : String(s.port);
  const live = lastStatus !== null && lastStatus.state !== 'idle';
  port.disabled = live;
  const inUse = view.portInUse ? `Now using port ${view.portInUse}. ` : '';
  $('port-hint').textContent = live
    ? `${inUse}Stop the broadcast to change it.`
    : `${inUse}Leave empty unless your school network needs a specific port.`;

  $<HTMLSelectElement>('set-toolbar').value = s.toolbarPosition;
  $<HTMLInputElement>('set-login').checked = s.launchAtLogin;
}

/** Reads the form; returns a plain-words problem instead if something's off. */
function readSettings(base: TeacherSettings): TeacherSettings | string {
  const pinOn = $<HTMLInputElement>('set-pin-on').checked;
  const pin = $<HTMLInputElement>('set-pin').value.trim();
  if (pinOn && !/^\d{4,8}$/.test(pin)) return 'The PIN must be 4 to 8 digits.';
  const portText = $<HTMLInputElement>('set-port').value.trim();
  const port = portText === '' ? 0 : Number(portText);
  if (!Number.isInteger(port) || port < 0 || port > 65535) {
    return 'The port must be a number up to 65535, or empty for automatic.';
  }
  return {
    ...base,
    quality: $<HTMLSelectElement>('set-quality').value as Quality,
    maxFps: Number($<HTMLSelectElement>('set-fps').value),
    pin: pinOn ? pin : null,
    lanIp: $<HTMLSelectElement>('set-network').value || null,
    port,
    toolbarPosition: $<HTMLSelectElement>('set-toolbar').value as ToolbarPosition,
    launchAtLogin: $<HTMLInputElement>('set-login').checked,
  };
}

function settingsError(message: string | null): void {
  $('settings-error').textContent = message ?? '';
  $('settings-error').hidden = !message;
}

async function openSettings(): Promise<void> {
  showError(null);
  settingsError(null);
  try {
    fillSettings(await getSettings());
  } catch (err) {
    if (err instanceof CompanionError) showView(err.problem);
    else showError("Couldn't load the settings. Please try again.");
    return;
  }
  viewBehindSettings = currentView;
  showView('settings');
  settingsOpen = true;
  $('set-quality').focus();
}

function closeSettings(): void {
  settingsOpen = false;
  showView(viewBehindSettings);
  $('settings-btn').focus();
}

async function submitSettings(): Promise<void> {
  if (!settingsView) return;
  const next = readSettings(settingsView.settings);
  if (typeof next === 'string') {
    settingsError(next);
    return;
  }
  const save = $<HTMLButtonElement>('settings-save');
  save.disabled = true;
  settingsError(null);
  try {
    settingsView = await saveSettings(next);
    closeSettings();
  } catch (err) {
    if (err instanceof ApiError && err.reason) settingsError(err.reason);
    else if (err instanceof CompanionError) showView(err.problem);
    else settingsError("Couldn't save. Please try again.");
  } finally {
    save.disabled = false;
  }
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

  $('settings-btn').innerHTML = icons.gear; // static icon markup from shared/icons
  $('settings-btn').addEventListener('click', openSettings);
  $('settings-cancel').addEventListener('click', closeSettings);
  $('settings-form').addEventListener('submit', (e) => {
    e.preventDefault();
    submitSettings();
  });
  $('set-pin-on').addEventListener('change', () => {
    const on = $<HTMLInputElement>('set-pin-on').checked;
    $('set-pin').hidden = !on;
    if (on) $('set-pin').focus();
  });
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && settingsOpen) closeSettings();
  });

  $('fix-btn').addEventListener('click', async () => {
    const fix = $('fix-btn').dataset.fix;
    if (!fix) return;
    try {
      await requestFix(fix);
    } catch {
      showError(
        "Couldn't open System Settings. Open it and choose Privacy & Security → Screen Recording.",
      );
    }
  });

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
