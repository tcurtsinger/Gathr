import { expect, test } from 'vitest';
import { render } from 'vitest-browser-react';
import { Markdown } from './Markdown';

// Vale's three (review of #33), and a footnote, whose definition renders apart from its reference.
const inputs = {
  'a numbered list': '1. First\n\n1. Second\n\n1. Third',
  'a reference link': 'Read [the notes][notes].\n\n[notes]: https://example.com/notes',
  'a fence with trailing text': '```txt\none\n``` not a closing fence\n\ntwo\n```\n\nAfter.',
  'a footnote': 'Said once.[^1]\n\nMore.\n\n[^1]: Where it was said.',
};

async function html(text: string, streaming: boolean) {
  const screen = await render(<Markdown text={text} streaming={streaming} />);
  const out = screen.container.innerHTML;
  await screen.unmount();
  return out;
}

for (const [name, text] of Object.entries(inputs)) {
  test(`${name} streams as the whole text renders, at every length`, async () => {
    for (let n = 1; n <= text.length; n++) {
      const part = text.slice(0, n);
      expect(await html(part, true), JSON.stringify(part)).toBe(await html(part, false));
    }
  });
}

test('a saved reply renders its whole text: the list counts on, the reference resolves', async () => {
  const list = await render(<Markdown text={inputs['a numbered list']} />);
  expect(list.container.querySelectorAll('ol')).toHaveLength(1);
  const link = await render(<Markdown text={inputs['a reference link']} />);
  expect(link.container.querySelector('a')?.getAttribute('href')).toBe('https://example.com/notes');
});
