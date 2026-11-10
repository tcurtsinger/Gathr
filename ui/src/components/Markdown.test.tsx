import { expect, test } from 'vitest';
import { render } from 'vitest-browser-react';
import type { Message, Turn } from '../api';
import { type ChatState, empty } from '../chat/state';
import { type Actions, Thread } from '../chat/Thread';

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
const cases = { ...inputs, 'all of them in one reply': Object.values(inputs).join('\n\n') };

const none = new Proxy({}, { get: () => () => {} }) as Actions;
const turn = (status: Turn['status']): Turn => ({ id: 't', status, why: null, ref: 'r', agent: null });
const saved = (text: string): ChatState => {
  const reply: Message = { id: 'm', turn: 't', time: '', role: 'assistant', kind: 'text', body: text };
  return { ...empty, messages: [reply], turns: { t: turn('done') } };
};
const streaming = (text: string): ChatState => ({
  ...empty,
  turns: { t: turn('running') },
  partial: { t: text },
});

function View({ state }: { state: ChatState }) {
  return (
    <Thread state={state} agents={[]} unsent={[]} running={false} actions={none} empty={false}>
      {null}
    </Thread>
  );
}
// Newlines between blocks are only whitespace; the saved render keeps them, the streamed one does not.
// A token React updated in place keeps an empty style attribute where a new one has none.
const markdown = (container: HTMLElement) =>
  container.querySelector('.markdown')?.innerHTML.replaceAll(/>\n+</g, '><').replaceAll(' style=""', '');

// Before Shiki answers, Streamdown draws each token with `--sdm-c: inherit`.
const coloured = (container: HTMLElement) =>
  !container.querySelector('.markdown pre span[style*="--sdm-c: inherit"]');

for (const [name, text] of Object.entries(cases)) {
  test(`${name}: a streamed reply ends as the saved reply renders`, async () => {
    const whole = await render(<View state={saved(text)} />);
    // Shiki colours a code block once its language loads; the stream then finds it coloured at once.
    await expect.poll(() => coloured(whole.container), { timeout: 5000 }).toBe(true);
    const expected = markdown(whole.container);
    await whole.unmount();

    const stream = await render(<View state={streaming(text.slice(0, 1))} />);
    for (let n = 8; n < text.length; n += 8)
      await stream.rerender(<View state={streaming(text.slice(0, n))} />);
    await stream.rerender(<View state={saved(text)} />);
    await expect.poll(() => markdown(stream.container)).toBe(expected);
  });
}
