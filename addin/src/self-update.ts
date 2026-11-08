/**
 * Office (especially on Mac) can keep showing an old cached copy of this
 * panel for a long time. On start we fetch version.json, bypassing every
 * cache; if it names a newer build than the one running, we reload once
 * from a fresh URL.
 */
declare const __BUILD_ID__: string;

const RELOAD_FLAG = 'sliderelay.reloadedFor';

export async function reloadIfOutdated(): Promise<void> {
  if (__BUILD_ID__.startsWith('dev-')) return; // local development
  try {
    const res = await fetch(`./version.json?t=${Date.now()}`, { cache: 'no-store' });
    if (!res.ok) return;
    const { build } = (await res.json()) as { build?: string };
    if (!build || build === __BUILD_ID__) return;
    // Only try once per new build, so a stale CDN can't cause a reload loop.
    if (sessionStorage.getItem(RELOAD_FLAG) === build) return;
    sessionStorage.setItem(RELOAD_FLAG, build);
    location.replace(`./taskpane.html?v=${encodeURIComponent(build)}`);
  } catch {
    /* offline or storage blocked — keep running the current copy */
  }
}
