import { ChevronRight } from 'lucide-react';
import { useContext, useEffect, useState } from 'react';
import { duration, type Work } from './fold';
import { Dot } from './Said';
import { FoundContext } from './TurnParts';

/**
 * Whether a turn's steps are open: closed at first (open in the agent's panel), and opened by a search hit on
 * something it wrote mid-work.
 */
export function useStepsOpen(work: Work | undefined, startOpen = false) {
  const found = useContext(FoundContext);
  const holds = !!found && !!work?.steps.some((s) => 'say' in s && s.say === found);
  const [open, setOpen] = useState(holds || startOpen);
  const [held, setHeld] = useState(holds);
  if (holds !== held) {
    setHeld(holds);
    if (holds) setOpen(true);
  }
  return [open, setOpen] as const;
}

/**
 * A turn's steps on its message's name line (0072): "· 10 steps · 6m 40s · 1 failed ›". It shows while the message
 * is pointed at or has focus, and always for a failed step or an agent still working (said.css); it opens the
 * steps under the name line.
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
  const pieces = [
    ...work.notes,
    work.running && work.since !== undefined ? <LiveTime key="t" since={work.since} /> : work.took,
    work.failed ? (
      <span key="f" className="steps-failed">
        {work.failed} failed
      </span>
    ) : undefined,
  ].filter(Boolean);
  const label = [...work.notes, work.running ? undefined : work.took, work.failed && `${work.failed} failed`];
  return (
    <button
      type="button"
      className="steps-button"
      aria-label={label.filter(Boolean).join(' · ')}
      aria-expanded={open}
      onClick={() => onOpen(!open)}
    >
      {pieces.map((piece, i) => (
        // biome-ignore lint/suspicious/noArrayIndexKey: the pieces keep their order
        <span key={i} className="steps-piece">
          <Dot />
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
