import { useRef, useState } from 'react';
import { expect, test } from 'vitest';
import { render } from 'vitest-browser-react';
import { useEdges } from './edges';

/** A scroller that gets its content after it mounted, as a thread's can (main's WebKit run, c301c38). */
function Scroller() {
  const ref = useRef<HTMLDivElement>(null);
  const [content, setContent] = useState(false);
  useEdges(ref);
  return (
    <>
      <button type="button" onClick={() => setContent(true)}>
        Fill
      </button>
      <div ref={ref} data-testid="scroller" style={{ height: 100, overflow: 'auto' }}>
        {content && <div style={{ height: 1000 }} />}
      </div>
    </>
  );
}

test('content that arrives after the scroller mounted sets its edges, with no scroll', async () => {
  const screen = await render(<Scroller />);
  const el = screen.getByTestId('scroller').element() as HTMLElement;
  await screen.getByRole('button', { name: 'Fill' }).click();
  await expect.poll(() => el.hasAttribute('data-fade-below')).toBe(true);
});
