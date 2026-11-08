// The big QR window the teacher shows the class. The link comes from the
// panel in the address (?url=...); the panel closes this window when asked
// or when the broadcast stops.
import '@sliderelay/shared/theme.css';
import './qr-dialog.css';
import { isStudentLink, qrSvg, shortLink } from './qr';

const url = new URLSearchParams(location.search).get('url');
const $ = (id: string) => document.getElementById(id) as HTMLElement;

if (isStudentLink(url)) {
  $('qr').append(qrSvg(url));
  $('link').textContent = shortLink(url);
}

function close(): void {
  try {
    Office.context.ui.messageParent('close');
  } catch {
    window.close();
  }
}

Office.onReady(() => {
  $('close-btn').addEventListener('click', close);
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') close();
  });
});
