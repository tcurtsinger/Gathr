// The marks' writer (0077): marking happens as the person scrolls and text streams, so the service is written at most
// once a second, with the newest mark of each thread; a mark never moves back, and a chat with no marks is seen up to
// when it first opened.
import { afterEach, expect, test, vi } from 'vitest';
import { isNew, Reads, WRITE_MS } from './reads';

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

function writes() {
  const sent: { topic: string; seen_us?: number; folded?: boolean }[][] = [];
  vi.stubGlobal('fetch', async (_: string, init?: RequestInit) => {
    sent.push(JSON.parse(String(init?.body)).reads);
    return new Response(JSON.stringify({ reads: [] }));
  });
  return sent;
}

test('a burst of marks is one write, and the next waits for the second to pass', async () => {
  vi.useFakeTimers();
  const sent = writes();
  const reads = new Reads('c');
  reads.take([{ topic: '', seen_us: 1_000, folded: null }], () => 0);
  for (let t = 2; t < 200; t++) reads.see('t:1', t);
  reads.see('', 5);
  reads.fold('t:1', true);
  expect(sent).toEqual([]);
  await vi.advanceTimersByTimeAsync(0);
  expect(sent).toEqual([
    [
      { topic: 't:1', seen_us: 199_000, folded: true },
      { topic: '', seen_us: 5_000 },
    ],
  ]);

  reads.see('t:1', 300);
  await vi.advanceTimersByTimeAsync(WRITE_MS - 1);
  expect(sent.length).toBe(1);
  await vi.advanceTimersByTimeAsync(1);
  expect(sent[1]).toEqual([{ topic: 't:1', seen_us: 300_000 }]);
});

test('a mark older than the one kept moves nothing and writes nothing', async () => {
  vi.useFakeTimers();
  const sent = writes();
  const reads = new Reads('c');
  reads.take([{ topic: 't:1', seen_us: 9_000, folded: null }], () => 0);
  reads.see('t:1', 8);
  reads.apply([{ topic: 't:1', seen_us: 7_000, folded: null }]);
  await vi.advanceTimersByTimeAsync(WRITE_MS);
  expect(sent).toEqual([]);
  expect(reads.seen('t:1')).toBe(9);
});

test("in a chat with marks, a thread with none is seen up to the main line's mark, and New counts from there", () => {
  const reads = new Reads('c');
  reads.take(
    [
      { topic: '', seen_us: 5_000_000, folded: null },
      { topic: 't:1', seen_us: 2_000_000, folded: null },
    ],
    () => 0,
  );
  // A thread an update named anew (0078): what it held before the main line's mark is seen.
  expect(reads.seen('b:new')).toBe(5_000);
  expect(reads.seen('t:1')).toBe(2_000);
  expect(reads.unseen('b:new', [4_000, 5_000, 6_000])).toBe(1);
  const left = reads.now.left;
  expect([4_000, 6_000].map((t) => isNew(left, 'b:new', t))).toEqual([false, true]);
});

test('a chat with no marks is seen up to when it first opened, and keeps each of its threads so', async () => {
  vi.useFakeTimers({ now: 50_000 });
  const sent = writes();
  const reads = new Reads('c');
  reads.take([], () => 60_000);
  expect(reads.seen('t:7')).toBe(60_000);
  reads.baseline(() => ['t:7', 'm:x']);
  reads.baseline(() => ['t:8']);
  await vi.advanceTimersByTimeAsync(0);
  expect(sent).toEqual([
    [
      { topic: '', seen_us: 60_000_000 },
      { topic: 't:7', seen_us: 60_000_000 },
      { topic: 'm:x', seen_us: 60_000_000 },
    ],
  ]);
});
