import { useEffect, useId, useRef, useState } from 'react';
import { Button } from '@/components/ui/button';
import { danger } from '../components/Confirm';
import { type BrowserPage, type PageDialog, say } from './pages';

/** Where it is from, as a browser names it: "example.com says". */
function heading(page: BrowserPage) {
  try {
    const host = new URL(page.url).host;
    if (host) return `${host} says`;
  } catch {}
  return 'This page says';
}

/**
 * A page's own alert, confirm, prompt or question on leaving, over the top of the page as a browser shows it for its
 * tab: never the window's modal, so a page can't lock the chat. It takes the keys only from the page or the Browser
 * tab's own row, and gives them back to the page (`back`) once answered.
 */
export function PageQuestion({
  page,
  dialog,
  back,
}: {
  page: BrowserPage;
  dialog: PageDialog;
  back: React.RefObject<HTMLDivElement | null>;
}) {
  const card = useRef<HTMLDivElement>(null);
  const field = useRef<HTMLInputElement>(null);
  const first = useRef<HTMLButtonElement>(null);
  const [text, setText] = useState(dialog.value);
  const titled = useId();
  const told = useId();
  const leaving = dialog.kind === 'beforeunload';
  const asks = dialog.kind !== 'alert';
  const message = leaving ? 'Changes you made may not be saved.' : dialog.message;

  useEffect(() => {
    const had = document.activeElement;
    if (had && card.current?.closest('.browser')?.contains(had)) (field.current ?? first.current)?.focus();
  }, []);

  const answer = (accept: boolean) => {
    const had = card.current?.contains(document.activeElement);
    say({ type: 'answer', page: page.id, accept, ...(accept && dialog.kind === 'prompt' && { text }) });
    if (had) back.current?.focus();
  };

  return (
    <div
      ref={card}
      role="alertdialog"
      aria-labelledby={titled}
      aria-describedby={message ? told : undefined}
      className="dialog confirm-dialog browser-question"
      onKeyDown={(e) => {
        if (e.key !== 'Escape') return;
        e.preventDefault();
        e.stopPropagation();
        answer(dialog.kind === 'alert');
      }}
    >
      <header className="dialog-header">
        <h2 id={titled}>{leaving ? 'Leave this page?' : heading(page)}</h2>
      </header>
      {message && (
        <p id={told} className="confirm-body muted">
          {message}
        </p>
      )}
      <form
        onSubmit={(e) => {
          e.preventDefault();
          answer(true);
        }}
      >
        {dialog.kind === 'prompt' && (
          <input
            ref={field}
            value={text}
            aria-labelledby={message ? told : titled}
            autoComplete="off"
            spellCheck={false}
            onChange={(e) => setText(e.target.value)}
            onFocus={(e) => e.currentTarget.select()}
          />
        )}
        <footer>
          {asks && (
            <Button
              ref={leaving ? first : undefined}
              type="button"
              size="sm"
              variant="outline"
              onClick={() => answer(false)}
            >
              {leaving ? 'Stay' : 'Cancel'}
            </Button>
          )}
          <Button
            ref={leaving ? undefined : first}
            type="submit"
            size="sm"
            variant={leaving ? 'outline' : 'default'}
            className={leaving ? danger : undefined}
          >
            {leaving ? 'Leave' : 'OK'}
          </Button>
        </footer>
      </form>
    </div>
  );
}
