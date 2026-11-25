// A long word in an agent's words (a path, a link, a hash) breaks where it must, so it never makes the chat scroll
// sideways at the narrowest window. The column has the chat's 24px sides (.room-scroll).
import { expect, test } from 'vitest';
import { page } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import { Markdown } from './Markdown';
import '../theme.css';
import '../app.css';

test.each([
  ['a word', `See ${'a'.repeat(120)}`],
  ['inline code', `See \`${'a'.repeat(120)}\``],
  ['a link', `See https://example.com/${'a'.repeat(120)}`],
  ['a heading', `## See ${'a'.repeat(120)}`],
  ['a list item', `- See ${'a'.repeat(120)}`],
])('%s stays inside a narrow column', async (_, text) => {
  await render(
    <div data-testid="column" style={{ width: 240, padding: '0 24px', overflow: 'auto' }}>
      <Markdown text={text} />
    </div>,
  );
  const box = page.getByTestId('column').element();
  await expect.poll(() => box.textContent).toContain('See');
  expect(box.scrollWidth, 'the column scrolls sideways').toBe(box.clientWidth);
});
