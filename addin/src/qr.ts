// Draws the viewer link as a QR code, in the panel itself (no app window).
import qrcode from 'qrcode-generator';

const SVG_NS = 'http://www.w3.org/2000/svg';

/** A crisp black-on-white QR code as an <svg> element, scaled by CSS. */
export function qrSvg(text: string): SVGSVGElement {
  const qr = qrcode(0, 'M');
  qr.addData(text);
  qr.make();
  const count = qr.getModuleCount();
  const quiet = 2; // white border so phones can find the code
  const size = count + quiet * 2;

  let path = '';
  for (let row = 0; row < count; row++) {
    for (let col = 0; col < count; col++) {
      if (qr.isDark(row, col)) path += `M${col + quiet} ${row + quiet}h1v1h-1z`;
    }
  }

  const svg = document.createElementNS(SVG_NS, 'svg');
  svg.setAttribute('viewBox', `0 0 ${size} ${size}`);
  svg.setAttribute('shape-rendering', 'crispEdges');
  svg.setAttribute('role', 'img');
  svg.setAttribute('aria-label', 'QR code for the viewer link');
  const bg = document.createElementNS(SVG_NS, 'rect');
  bg.setAttribute('width', String(size));
  bg.setAttribute('height', String(size));
  bg.setAttribute('fill', '#ffffff');
  const dots = document.createElementNS(SVG_NS, 'path');
  dots.setAttribute('d', path);
  dots.setAttribute('fill', '#000000');
  svg.append(bg, dots);
  return svg;
}

/** The link without "http://", so it's shorter to read out and type. */
export function shortLink(url: string): string {
  return url.replace(/^https?:\/\//, '');
}

/** Only links SlideRelay makes: plain http(s) addresses. */
export function isViewerLink(value: string | null): value is string {
  if (!value) return false;
  try {
    const u = new URL(value);
    return u.protocol === 'http:' || u.protocol === 'https:';
  } catch {
    return false;
  }
}
