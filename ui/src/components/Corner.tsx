import { CircleAlert, LoaderCircle, X } from 'lucide-react';
import {
  type RefObject,
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
  useSyncExternalStore,
} from 'react';
import { createPortal } from 'react-dom';
import { Button } from '@/components/ui/button';
import type { Failure } from '../api';
import { IconButton } from './IconButton';
import './corner.css';

/** `calm`: something under way, not a failure (an engine downloading), shown with a turning mark. */
type Told = Pick<Failure, 'error' | 'why' | 'ref'> & { calm?: boolean };
interface Action {
  label: string;
  run: () => void;
}
interface Card {
  failure: Told;
  action?: Action;
}

// The cards by the place that told them, oldest first; and the modal dialogs open, the top one last.
let cards = new Map<string, Card>();
let modals: HTMLDialogElement[] = [];
const heard = new Set<() => void>();
const changed = () => {
  for (const hear of heard) hear();
};
const subscribe = (hear: () => void) => {
  heard.add(hear);
  return () => heard.delete(hear);
};

function forget(key: string) {
  if (!cards.has(key)) return;
  cards = new Map(cards);
  cards.delete(key);
  changed();
}

/**
 * Tells a failure in the corner while it is set: a new one from the same place replaces its card, and the card goes
 * when the failure is unset or the place goes. The person can close it before that.
 */
export function useCard(failure: Told | null | undefined, action?: Action) {
  const key = useId();
  const latest = useRef(action);
  latest.current = action;
  useEffect(() => {
    if (!failure) return;
    const label = latest.current?.label;
    cards = new Map(cards);
    cards.delete(key);
    cards.set(key, { failure, action: label ? { label, run: () => latest.current?.run() } : undefined });
    changed();
    return () => forget(key);
  }, [key, failure]);
}

/** A modal dialog makes the page behind it inert: while one is open, the cards show inside it (Dialog.tsx). */
export function useModal(ref: RefObject<HTMLDialogElement | null>, open: boolean) {
  useEffect(() => {
    const dialog = ref.current;
    if (!dialog || !open) return;
    modals = [...modals, dialog];
    changed();
    return () => {
      modals = modals.filter((d) => d !== dialog);
      changed();
    };
  }, [ref, open]);
}

/** The one place failures show: small cards in the window's bottom-left corner, the newest nearest it. */
export function Corner({ onReference }: { onReference: (ref: string) => void }) {
  const shown = useSyncExternalStore(subscribe, () => cards);
  const top = useSyncExternalStore(subscribe, () => modals.at(-1));
  const [host] = useState(() => {
    const div = document.createElement('div');
    div.className = 'corner';
    return div;
  });
  useLayoutEffect(() => {
    (top ?? document.body).append(host);
  }, [host, top]);
  useEffect(() => () => host.remove(), [host]);
  return createPortal(
    [...shown].map(([key, { failure, action }]) => (
      <div key={key} className="corner-card" role={failure.calm ? 'status' : 'alert'}>
        {failure.calm ? (
          <LoaderCircle className="corner-mark corner-calm" aria-hidden="true" />
        ) : (
          <CircleAlert className="corner-mark" aria-hidden="true" />
        )}
        <div className="corner-words">
          <strong>{failure.error}</strong>
          {failure.why && <span>{failure.why}</span>}
          {(failure.ref || action) && (
            <div className="corner-foot">
              {failure.ref && <Reference reference={failure.ref} onOpen={onReference} />}
              {action && (
                <Button type="button" variant="outline" size="xs" onClick={action.run}>
                  {action.label}
                </Button>
              )}
            </div>
          )}
        </div>
        <IconButton aria-label="Close" className="corner-x" onClick={() => forget(key)}>
          <X />
        </IconButton>
      </div>
    )),
    host,
  );
}

/** An error's reference, which opens it in Diagnostics. */
function Reference({ reference, onOpen }: { reference: string; onOpen: (ref: string) => void }) {
  return (
    <button
      type="button"
      className="ref"
      onClick={() => onOpen(reference)}
      aria-label={`Reference ${reference}`}
    >
      {reference}
    </button>
  );
}
