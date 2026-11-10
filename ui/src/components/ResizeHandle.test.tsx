import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import { userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import { ResizeHandle } from './ResizeHandle';
import '../theme.css';

const name = '--test-pane-width';

/** A pane 300px wide unless the handle's variable says otherwise, with its handle on `side`. */
function pane(side: 'left' | 'right', max = 400, key = 'first') {
  return (
    <div key={key} style={{ position: 'relative', width: `var(${name}, 300px)`, height: 200 }}>
      <ResizeHandle label="Resize pane" side={side} name={name} min={200} max={() => max} />
    </div>
  );
}

/** Writes the window's cookie as any port of this host could. */
function cookie(value: string) {
  // biome-ignore lint/suspicious/noDocumentCookie: the test plays a window on another port.
  document.cookie = `gathr.width${name}=${value}; path=/; max-age=${value ? 60 : 0}`;
}

beforeEach(() => {
  cookie('');
  document.documentElement.style.removeProperty(name);
});
afterEach(() => vi.restoreAllMocks());

test('a width kept while the service ran on another port is used at start', async () => {
  // localStorage is kept per origin, so per port; a cookie is kept per host, which every port reads.
  cookie('350');
  const screen = await render(pane('right'));
  await expect
    .element(screen.getByRole('separator', { name: 'Resize pane' }))
    .toHaveAttribute('aria-valuenow', '350');
});

test('keys move a right-edge handle 10px, 40px with Shift, and Home and End go to the limits', async () => {
  const screen = await render(pane('right'));
  const handle = screen.getByRole('separator', { name: 'Resize pane' });
  await expect.element(handle).toHaveAttribute('aria-valuenow', '300');
  handle.element().focus();

  await userEvent.keyboard('{ArrowRight}');
  await expect.element(handle).toHaveAttribute('aria-valuenow', '310');
  await userEvent.keyboard('{Shift>}{ArrowLeft}{/Shift}');
  await expect.element(handle).toHaveAttribute('aria-valuenow', '270');
  await userEvent.keyboard('{End}');
  await expect.element(handle).toHaveAttribute('aria-valuenow', '400');
  await userEvent.keyboard('{ArrowRight}');
  await expect.element(handle).toHaveAttribute('aria-valuenow', '400');
  await userEvent.keyboard('{Home}');
  await expect.element(handle).toHaveAttribute('aria-valuenow', '200');
  await userEvent.keyboard('{ArrowLeft}');
  await expect.element(handle).toHaveAttribute('aria-valuenow', '200');
});

test('a left-edge handle grows its pane as it moves left', async () => {
  const screen = await render(pane('left'));
  const handle = screen.getByRole('separator', { name: 'Resize pane' });
  handle.element().focus();
  await userEvent.keyboard('{ArrowLeft}');
  await expect.element(handle).toHaveAttribute('aria-valuenow', '310');
});

test('a drag follows the pointer within the limits, and the width is kept for the next start', async () => {
  // Synthetic pointers have no active pointer to capture; real ones are in the PR's live proof.
  vi.spyOn(HTMLElement.prototype, 'setPointerCapture').mockImplementation(() => {});
  const screen = await render(pane('left', 380));
  const handle = screen.getByRole('separator', { name: 'Resize pane' });
  const el = handle.element();
  const pointer = (type: string, clientX: number) =>
    el.dispatchEvent(new PointerEvent(type, { clientX, button: 0, pointerId: 7, bubbles: true }));

  pointer('pointerdown', 500);
  pointer('pointermove', 450);
  await expect.element(handle).toHaveAttribute('aria-valuenow', '350');
  pointer('pointermove', 300);
  await expect.element(handle).toHaveAttribute('aria-valuenow', '380');
  pointer('pointerup', 300);
  pointer('pointermove', 600);
  await expect.element(handle).toHaveAttribute('aria-valuenow', '380');

  // A new start: nothing on the root yet, only what the window kept.
  document.documentElement.style.removeProperty(name);
  await screen.rerender(pane('left', 380, 'again'));
  await expect
    .element(screen.getByRole('separator', { name: 'Resize pane' }))
    .toHaveAttribute('aria-valuenow', '380');
});
