import type { ThreadMessageLike } from '@assistant-ui/react';
import { createContext, useContext, useEffect, useMemo, useSyncExternalStore } from 'react';
import { api, type Failure, type Read } from '../api';
import { listen } from '../live';
import { report } from '../report';
import type { Custom, Timeline } from './messages';

/** How often at most the window writes its marks (0077): marking happens as the person scrolls and text streams. */
export const WRITE_MS = 1000;

/**
 * What the person had seen when they came back to the chat (0077): the New line and each reply's New are drawn from it,
 * so they hold while the person reads. Taken when the chat opens and when the window comes back to the front; what
 * arrives after that is newer than `at`, and the line at the foot points to it instead.
 */
export interface Left {
  marks: Map<string, number>;
  /** A thread with no mark: seen up to here. */
  floor: number;
  at: number;
}

/** Said after the person left, and before they came back: New. */
export const isNew = (left: Left | undefined, topic: string, t: number) =>
  !!left && t > (left.marks.get(topic) ?? left.floor) && t <= left.at;

/** What a drawn item is to the marks: its thread ('' for the main line), when it was said, and whether it is theirs. */
interface Seen {
  topic: string;
  reply: boolean;
  t: number;
  mine: boolean;
  item: ThreadMessageLike;
}

const own = (m: ThreadMessageLike) => (m.metadata?.custom ?? {}) as Custom;
const noop = () => () => {};

/**
 * One chat's marks (0077), kept by the service: per thread, the newest message the person has seen there, and their own
 * fold. Seeing moves a mark here at once, with no redraw of the chat; the service is written at most once a second.
 * Folds and fold lines, the line at the foot and each reply's New read it through `ReadsContext`.
 */
export class Reads {
  private marks = new Map<string, number>();
  /** The person's own folds, as kept: a thread's open or folded wins over Gathr's rule. */
  readonly folds = new Map<string, boolean>();
  /** Before the chat has loaded nothing counts as new. */
  private floor = Number.POSITIVE_INFINITY;
  /**
   * What the chat is drawn from, new with each change to it: what the person had seen when they came back, and the
   * folds kept for them.
   */
  now: { left?: Left; folds: Map<string, boolean> } = { folds: new Map() };
  /** Drawn items with any part in view, by id: the line at the foot names none of them. */
  private shown = new Set<string>();
  private loaded = false;
  private taken: Read[] | undefined;
  private base = 0;
  private based = false;
  timeline: Timeline | undefined;
  private info: { of?: Timeline; by: Map<string, Seen> } = { by: new Map() };
  private pending = new Map<string, { seen_us?: number; folded?: boolean }>();
  private sending = new Set<string>();
  private timer = 0;
  private last = 0;
  private marked = new Set<() => void>();
  private changes = 0;
  private footFor: { of?: Timeline; at: number; id: string } = { at: -1, id: '' };
  private drawn = new Set<() => void>();
  /** A fold from another window: the open thread this window holds for it gives way. */
  onFold?: (topic: string) => void;

  constructor(private chat: string) {}

  /** Seen up to here in a thread ('' for the main line). */
  seen = (topic: string) => this.marks.get(topic) ?? this.floor;

  /** The chat's marks as its history read them. A chat with none yet is seen up to now (an old chat is not new). */
  take(rows: Read[], newest: number) {
    if (rows === this.taken) return;
    this.taken = rows;
    if (this.loaded) return this.apply(rows);
    this.loaded = true;
    for (const r of rows) {
      this.marks.set(r.topic, r.seen_us / 1000);
      if (r.folded !== null) this.folds.set(r.topic, r.folded);
    }
    this.now = { ...this.now, folds: new Map(this.folds) };
    this.base = Math.max(Date.now(), newest);
    this.floor = rows.length ? 0 : this.base;
    this.arrive();
  }

  /** The first open of a chat with no marks: every thread there is kept as seen up to then. */
  baseline(topics: Iterable<string>) {
    if (this.floor === 0 || !this.loaded || this.based) return;
    this.based = true;
    for (const topic of ['', ...topics]) {
      if (this.marks.has(topic)) continue;
      this.marks.set(topic, this.base);
      this.queue(topic, { seen_us: this.base * 1000 });
    }
  }

  /** The person came back: what is new is reckoned from what they had seen until now. */
  arrive() {
    const at = Math.max(Date.now(), this.base);
    this.now = { ...this.now, left: { marks: new Map(this.marks), floor: this.floor, at } };
    this.tell(this.drawn);
  }

  /** What shows now, and which of it is seen: half of it in view, with the window in front. Told once, whatever moved. */
  view(ids: Set<string>, seen: string[]) {
    const info = this.items();
    let moved = false;
    for (const id of seen) {
      const s = info.get(id);
      if (s && !s.mine) moved = this.move(s.reply ? s.topic : '', s.t) || moved;
    }
    const same = ids.size === this.shown.size && [...ids].every((id) => this.shown.has(id));
    if (!same) this.shown = ids;
    if (moved || !same) this.tell(this.marked);
  }

  /** Seen up to `t` in a thread: a mark never moves back. */
  see(topic: string, t: number) {
    if (this.move(topic, t)) this.tell(this.marked);
  }

  private move(topic: string, t: number) {
    if (!(t > this.seen(topic))) return false;
    this.marks.set(topic, t);
    this.queue(topic, { seen_us: Math.round(t * 1000) });
    return true;
  }

  /** The person's own fold (true) or open of a thread, kept. */
  fold(topic: string, folded: boolean) {
    this.folds.set(topic, folded);
    this.queue(topic, { folded });
  }

  /** Marks kept by another window, or by this one, as the service sends them: newer ones only, and their folds. */
  apply(rows: Read[]) {
    let moved = false;
    let folded = false;
    for (const r of rows) {
      const t = r.seen_us / 1000;
      if (r.seen_us && t > this.seen(r.topic)) {
        this.marks.set(r.topic, t);
        moved = true;
      }
      // A fold of this window's still on its way is the person's latest.
      const mine = this.pending.get(r.topic)?.folded !== undefined || this.sending.has(r.topic);
      if (r.folded === null || mine || this.folds.get(r.topic) === r.folded) continue;
      this.folds.set(r.topic, r.folded);
      this.onFold?.(r.topic);
      folded = true;
    }
    if (moved) this.tell(this.marked);
    if (folded) {
      this.now = { ...this.now, folds: new Map(this.folds) };
      this.tell(this.drawn);
    }
  }

  /** How many of a thread's replies, by when each was said, are newer than its mark. */
  unseen = (topic: string, times: number[]) => {
    const seen = this.seen(topic);
    return times.reduce((n, t) => (t > seen ? n + 1 : n), 0);
  };

  /**
   * The newest reply of someone else's not seen, with words, whose place isn't in view: its own block, or its thread's
   * main message while the thread is folded. '' for none.
   */
  foot = () => {
    const timeline = this.timeline;
    if (!timeline) return '';
    // Asked by each draw and each change: worked out again only when the marks, the view or the chat changed.
    if (this.footFor.of === timeline && this.footFor.at === this.changes) return this.footFor.id;
    const id = this.newestUnseen(timeline);
    this.footFor = { of: timeline, at: this.changes, id };
    return id;
  };

  private newestUnseen(timeline: Timeline) {
    const drawn = new Set(timeline.items.map((m) => m.id));
    const mainOf = new Map<string, string>();
    for (const m of timeline.items) {
      const o = own(m);
      if (o.topic && !o.reply && !o.earlier && !o.box) mainOf.set(o.topic, m.id as string);
    }
    let best: Seen | undefined;
    for (const s of this.items().values()) {
      if (!s.reply || s.mine || !(s.t > this.seen(s.topic)) || (best && s.t < best.t)) continue;
      if (!s.item.content?.length) continue;
      const place = drawn.has(s.item.id) ? s.item.id : mainOf.get(s.topic);
      if (place && this.shown.has(place as string)) continue;
      best = s;
    }
    return (best?.item.id as string | undefined) ?? '';
  }

  item = (id: string) => this.items().get(id)?.item;

  subscribe = (fn: () => void) => {
    this.marked.add(fn);
    return () => this.marked.delete(fn);
  };

  subscribeDrawn = (fn: () => void) => {
    this.drawn.add(fn);
    return () => this.drawn.delete(fn);
  };

  /** Writes what waits now, as when the chat closes. */
  flush = () => {
    clearTimeout(this.timer);
    this.timer = 0;
    if (!this.pending.size) return;
    this.last = Date.now();
    const sent = [...this.pending].map(([topic, v]) => ({ topic, ...v }));
    const folds = sent.flatMap((r) => (r.folded === undefined ? [] : [r.topic]));
    for (const topic of folds) this.sending.add(topic);
    this.pending.clear();
    const done = () => {
      for (const topic of folds) this.sending.delete(topic);
    };
    api.reads(this.chat, sent).then(done, (failure: Failure) => {
      done();
      // Nothing to show the person: the next mark of the same thread writes it again.
      report({ name: 'gathr.ui.reads', error: failure.error, attrs: { ref: failure.ref, why: failure.why } });
    });
  };

  private queue(topic: string, change: { seen_us?: number; folded?: boolean }) {
    this.pending.set(topic, { ...this.pending.get(topic), ...change });
    if (this.timer) return;
    this.timer = window.setTimeout(this.flush, Math.max(0, this.last + WRITE_MS - Date.now()));
  }

  private tell(listeners: Set<() => void>) {
    if (listeners === this.marked) this.changes += 1;
    for (const fn of listeners) fn();
  }

  /** Each drawn block by id, as the marks read it: built again only when the chat is. */
  private items() {
    const timeline = this.timeline;
    if (!timeline || this.info.of === timeline) return this.info.by;
    const by = new Map<string, Seen>();
    for (const item of timeline.all) {
      const o = own(item);
      const t = Date.parse(o.time ?? '');
      if (Number.isNaN(t) || o.earlier || o.box || o.newLine) continue;
      by.set(item.id as string, {
        topic: o.topic ?? '',
        reply: !!o.reply,
        t,
        mine: item.role === 'user',
        item,
      });
    }
    this.info = { of: timeline, by };
    return by;
  }
}

export const ReadsContext = createContext<Reads | null>(null);

/** A chat's marks: kept live from the socket, written when the chat closes or the page goes. */
export function useReads(chat: string): Reads {
  const reads = useMemo(() => new Reads(chat), [chat]);
  useEffect(() => {
    const stop = listen((e) => {
      if (e.type === 'reads' && e.chat === chat) reads.apply(e.reads);
    });
    let away = false;
    const blur = () => {
      away = true;
    };
    const focus = () => {
      if (away) reads.arrive();
      away = false;
    };
    window.addEventListener('blur', blur);
    window.addEventListener('focus', focus);
    window.addEventListener('pagehide', reads.flush);
    return () => {
      stop();
      window.removeEventListener('blur', blur);
      window.removeEventListener('focus', focus);
      window.removeEventListener('pagehide', reads.flush);
      reads.flush();
    };
  }, [chat, reads]);
  return reads;
}

/** How many of a folded thread's replies are newer than its mark: its fold line says so in blue. */
export function useUnseen(topic: string, times: number[] | undefined): number {
  const reads = useContext(ReadsContext);
  return useSyncExternalStore(reads?.subscribe ?? noop, () =>
    reads && times ? reads.unseen(topic, times) : 0,
  );
}

/** A reply said while the person was away: its name line says New until they come back again. */
export function useIsNew(topic: string | undefined, time: string | undefined): boolean {
  const reads = useContext(ReadsContext);
  return useSyncExternalStore(reads?.subscribeDrawn ?? noop, () =>
    isNew(reads?.now.left, topic ?? '', Date.parse(time ?? '')),
  );
}
