import { ChevronRight } from 'lucide-react';
import { createContext, useEffect, useState } from 'react';
import { duration, type Work } from './fold';
import { revealOpened } from './reveal';

/**
 * The timeline's components drawn in the agent's panel (0038): only there does a message show its steps and the task
 * it worked on (the owner's call of 2026-11-18).
 */
export const InPanel = createContext(false);

/**
 * A turn's steps on its message's name line in the agent's panel (0072): "· 10 steps · 6m 40s · 1 failed ›". It shows
 * while the message is pointed at or has focus, and always for a failed or refused step or an agent still working
 * (said.css); it opens the steps under the name line.
 */
export function StepsButton({
  work,
  open,
  onOpen,
}: {
  work: Work;
  open: boolean;
  onOpen: (open: boolean) => void;
}) {
  const bad = [work.failed && `${work.failed} failed`, work.refused && `${work.refused} refused`];
  const pieces = [
    ...work.notes,
    work.running && work.since !== undefined ? <LiveTime key="t" since={work.since} /> : work.took,
    ...bad.map(
      (b) =>
        b && (
          <span key={b} className="steps-failed">
            {b}
          </span>
        ),
    ),
  ].filter(Boolean);
  const label = [...work.notes, work.running ? undefined : work.took, ...bad];
  return (
    <button
      type="button"
      className="steps-button"
      aria-label={label.filter(Boolean).join(' · ')}
      aria-expanded={open}
      onClick={(e) => {
        onOpen(!open);
        const block = e.currentTarget.closest('.said');
        if (!open && block) revealOpened(e.currentTarget, block);
      }}
    >
      {pieces.map((piece, i) => (
        // biome-ignore lint/suspicious/noArrayIndexKey: the pieces keep their order
        <span key={i} className="steps-piece">
          {piece}
        </span>
      ))}
      <ChevronRight className="steps-chevron" aria-hidden="true" />
    </button>
  );
}

/** How long a running turn has run, redrawn each second: "4m 02s", after `before`; nothing under a second. */
export function LiveTime({ since, before = '' }: { since: number; before?: string }) {
  const [now, setNow] = useState(Date.now);
  useEffect(() => {
    const tick = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(tick);
  }, []);
  const took = duration(now - since, 'second');
  return took ? `${before}${took}` : null;
}
