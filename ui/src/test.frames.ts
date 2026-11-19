// Playwright's WebKit for Windows can stop giving a page animation frames for many seconds while its timers run
// on time (microsoft/playwright#43818, closed as not planned). A race in WebKit's Windows run loop leaves a spent
// timer in its queue, and the 16 ms timer that drives frames waits behind it until JavaScriptCore next sets its
// own timer. Playwright waits for two frames before every click and hover, so that action times out. The run loop
// and the timer are the Windows port's own (RunLoopWin.cpp, DisplayRefreshMonitorWin.cpp): the Mac's WebKit has
// neither, so the fault is this test engine's alone.
//
// Four times a second the watch asks for one frame. When none has come for two seconds, through which its own
// timer was on time, it makes enough garbage for JavaScriptCore to set its timer again, which ends the stall, and
// until a frame comes it repaints a 1px mark in vitest's own page, never the test's: a repaint is drawn without
// that timer.
import { server } from 'vitest/browser';

const every = 250;
// Slow is not stuck: on a busy PC a page's frames can take a second to come. A stall left alone lasts 20 or more.
const stuck = 2000;

type Page = {
  now: () => number;
  // Asks for one animation frame.
  frame: (came: () => void) => void;
  stalled: (waited: number) => void;
  repaint: () => void;
};

/** One tick of the watch's timer. */
export function frameWatch(page: Page): () => void {
  let asked: number | undefined;
  let since = 0;
  let ticked = page.now();
  let stalled = false;
  return () => {
    const at = page.now();
    const onTime = at - ticked < every * 2;
    ticked = at;
    if (asked === undefined) {
      asked = at;
      since = at;
      page.frame(() => {
        asked = undefined;
        stalled = false;
      });
      return;
    }
    if (!stalled) {
      // A late tick is a busy PC: frames are late then too, and nothing is stuck. The two seconds start again.
      if (!onTime) since = at;
      if (at - since < stuck) return;
      stalled = true;
      page.stalled(at - asked);
    }
    page.repaint();
  };
}

if (server.browser === 'webkit' && server.platform === 'win32') {
  // The real ones, kept before a test's fake timers can replace them.
  const frame = window.requestAnimationFrame.bind(window);
  const now = performance.now.bind(performance);
  const above = window.parent.document;
  let mark: HTMLElement | undefined;
  const tick = frameWatch({
    now,
    frame: (came) =>
      frame(() => {
        mark?.remove();
        mark = undefined;
        came();
      }),
    stalled: (waited) => {
      console.warn(
        `WebKit gave this page no animation frame for ${Math.round(waited)} ms: src/test.frames.ts`,
      );
      const garbage: object[] = [];
      for (let i = 0; i < 2_000_000; i++) garbage.push({ i });
    },
    repaint: () => {
      if (!mark) {
        mark = above.createElement('div');
        mark.style.cssText = 'position:fixed;left:0;top:0;width:1px;height:1px;pointer-events:none';
        above.body.append(mark);
      }
      mark.style.background = mark.style.background ? '' : '#000';
    },
  });
  window.setInterval(tick, every);
  // The mark is in a page that outlives this one.
  window.addEventListener('pagehide', () => mark?.remove());
}
