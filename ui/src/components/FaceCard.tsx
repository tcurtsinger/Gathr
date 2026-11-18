import { PreviewCard } from '@base-ui/react/preview-card';
import { createContext, type ReactNode, useContext, useMemo, useRef, useState } from 'react';
import { type Card, cardLine } from './marks';
import { StateMark } from './StateMark';

interface Cards {
  /** Shows the card at the face, or keeps it there with what it says now. */
  show: (face: Element, card: Card) => void;
  /** Hides it, if it is this face's. */
  hide: (face: Element) => void;
}

const CardsContext = createContext<Cards | null>(null);
export const useCards = () => useContext(CardsContext);

/**
 * A face's card (0071): who, and its state with what and since when. Information, not help, so it shows on hover;
 * it is the conversation map's card (the registry's Base UI preview card), one for every face under it, put at
 * whichever face was pointed at or focused.
 */
export function FaceCards({ children }: { children: ReactNode }) {
  const [shown, setShown] = useState<{ face: Element; card: Card } | null>(null);
  // What it said, while it closes.
  const last = useRef<Card | null>(null);
  if (shown) last.current = shown.card;
  const cards = useMemo<Cards>(
    () => ({
      show: (face, card) => setShown((s) => (s?.face === face && same(s.card, card) ? s : { face, card })),
      hide: (face) => setShown((s) => (s?.face === face ? null : s)),
    }),
    [],
  );
  const card = shown?.card ?? last.current;
  return (
    <CardsContext value={cards}>
      {children}
      <PreviewCard.Root
        open={!!shown}
        // The face says when it is pointed at or focused; the card's own hover rules know no trigger here.
        onOpenChange={(open, { reason }) => !open && reason === 'escape-key' && setShown(null)}
      >
        <PreviewCard.Portal>
          <PreviewCard.Positioner
            anchor={shown?.face}
            side="top"
            align="start"
            sideOffset={8}
            collisionPadding={8}
          >
            <PreviewCard.Popup className="face-card">
              {card && (
                <>
                  <span className="face-card-name">
                    <b>{card.name}</b>
                    {card.role && <span>{card.role}</span>}
                  </span>
                  <span className="face-card-state">
                    {card.state && <StateMark state={card.state} size={10} />}
                    {card.state ? cardLine(card.state, [card.about], card.since) : 'Removed'}
                  </span>
                </>
              )}
            </PreviewCard.Popup>
          </PreviewCard.Positioner>
        </PreviewCard.Portal>
      </PreviewCard.Root>
    </CardsContext>
  );
}

const same = (a: Card, b: Card) =>
  a.name === b.name && a.role === b.role && a.state === b.state && a.about === b.about && a.since === b.since;
