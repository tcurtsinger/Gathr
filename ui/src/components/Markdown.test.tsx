import { expect, test } from 'vitest';
import { render } from 'vitest-browser-react';
import { Markdown } from './Markdown';

// Vale's inputs (reviews of #33), footnotes, and blocks whose meaning a later line changes.
const inputs = {
  'a numbered list': '1. First\n\n1. Second\n\n1. Third',
  'a numbered list after text': 'Intro.\n\nMore.\n\n1. First\n\n1. Second\n\n1. Third',
  'a reference link': 'Read [the notes][notes].\n\n[notes]: https://example.com/notes',
  'a fence with trailing text': '```txt\none\n``` not a closing fence\n\ntwo\n```\n\nAfter.',
  'a footnote': 'Said once.[^1]\n\nMore.\n\n[^1]: Where it was said.',
  'a reference defined in a quote':
    'Read [the notes][notes].\n\n> [notes]: https://example.com/notes\n\nAfter.',
  'a footnote defined in a list': 'Said once.[^1]\n\n- item\n\n  [^1]: Where it was said.\n\nAfter.',
  'a heading underlined': 'Intro.\n\nTitle\n-----\n\nBody.',
  'a list then its item': 'Para\n- one\n\n  still one\n- two\n\nDone.',
  'a table forming': 'Before.\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\nAfter.',
  'a lazy quote': '> quoted\nlazy line\n\nOutside.',
  'a fence with blank lines': 'Code:\n\n~~~\na\n\n\nb\n~~~\nEnd.',
};

/** The whole text as a saved reply renders it. */
async function whole(text: string) {
  const screen = await render(<Markdown text={text} />);
  const out = screen.container.innerHTML;
  await screen.unmount();
  return out;
}

// Without the inputs that render whole, so the rest stream as blocks together.
const {
  'a footnote': _f,
  'a reference defined in a quote': _q,
  'a footnote defined in a list': _l,
  ...together
} = inputs;
const cases = { ...inputs, 'all of them in one reply': Object.values(together).join('\n\n') };

for (const [name, text] of Object.entries(cases)) {
  test(`${name} streams as the whole text renders, at every length`, async () => {
    const stream = await render(<Markdown text={text.slice(0, 1)} streaming />);
    for (let n = 1; n <= text.length; n++) {
      const part = text.slice(0, n);
      await stream.rerender(<Markdown text={part} streaming />);
      expect(stream.container.innerHTML, JSON.stringify(part)).toBe(await whole(part));
    }
  });
}

test('a saved reply renders its whole text: the list counts on, the reference resolves', async () => {
  const list = await render(<Markdown text={inputs['a numbered list']} />);
  expect(list.container.querySelectorAll('ol')).toHaveLength(1);
  const link = await render(<Markdown text={inputs['a reference link']} />);
  expect(link.container.querySelector('a')?.getAttribute('href')).toBe('https://example.com/notes');
});
