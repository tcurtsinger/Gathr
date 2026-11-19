// The frame watch (test.frames.ts), with a clock and frames moved by hand: Playwright's WebKit for Windows
// can't be made to stall when asked.
import { expect, test } from 'vitest';
import { frameWatch } from './test.frames';

function page() {
  let at = 0;
  let came: (() => void) | undefined;
  const did: string[] = [];
  const tick = frameWatch({
    now: () => at,
    frame: (done) => {
      came = done;
    },
    stalled: (waited) => did.push(`stalled after ${waited}`),
    repaint: () => did.push('repaint'),
  });
  return {
    did,
    // The timer fires `ms` after its last tick, `times` over; before each, the frame asked for comes, or not.
    tick(ms: number, frame: 'a frame came' | 'no frame', times = 1) {
      for (let i = 0; i < times; i++) {
        if (frame === 'a frame came') came?.();
        at += ms;
        tick();
      }
    },
  };
}

test('a page that draws is left alone', () => {
  const p = page();
  p.tick(250, 'a frame came', 20);
  expect(p.did).toEqual([]);
});

test('two seconds with no frame while the timer is on time: it says so once, and repaints until a frame comes', () => {
  const p = page();
  // The first tick asks, and seven more make 1.75 s.
  p.tick(250, 'no frame', 8);
  expect(p.did).toEqual([]);
  p.tick(250, 'no frame', 2);
  expect(p.did).toEqual(['stalled after 2000', 'repaint', 'repaint']);
  p.tick(250, 'a frame came', 2);
  expect(p.did).toEqual(['stalled after 2000', 'repaint', 'repaint']);
});

test('a stall after one that ended is said again', () => {
  const p = page();
  p.tick(250, 'no frame', 9);
  // This tick asks again, so the next stall is two seconds from here.
  p.tick(250, 'a frame came');
  p.tick(250, 'no frame', 8);
  expect(p.did).toEqual(['stalled after 2000', 'repaint', 'stalled after 2000', 'repaint']);
});

test('a late timer is a busy PC, not a stall: the two seconds start again from it', () => {
  const p = page();
  p.tick(250, 'no frame', 7);
  p.tick(900, 'no frame');
  p.tick(250, 'no frame', 7);
  expect(p.did).toEqual([]);
  p.tick(250, 'no frame');
  expect(p.did).toEqual(['stalled after 4400', 'repaint']);
});
