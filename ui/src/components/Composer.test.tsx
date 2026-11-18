import { useState } from 'react';
import { expect, test } from 'vitest';
import { page, server, userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import { Composer, type Person } from './Composer';
import type { Picked } from './mention';

/** Made-up agents the box can name: two in the chat, one not in it yet. */
const people: Person[] = [
  { id: 'a', name: 'Ann', meta: 'Supervisor' },
  { id: 'b', name: 'Abe', meta: 'Member' },
  { id: 'c', name: 'Bo', out: true },
];

/** The composer as a chat holds it, at a chat's foot: sending clears the draft and lists what was sent. */
function Chat({ running = false }: { running?: boolean }) {
  const [draft, setDraft] = useState('');
  const [picks, setPicks] = useState<Picked[]>([]);
  const [sent, setSent] = useState<string[]>([]);
  const [stopped, setStopped] = useState(false);
  return (
    // Room above for the "@" list, as the chat gives it.
    <div style={{ paddingTop: 240 }}>
      <Composer
        hint="Message Ann"
        people={people}
        draft={draft}
        picks={picks}
        onDraft={(text, p) => {
          setDraft(text);
          setPicks(p);
        }}
        onSend={() => {
          setSent([...sent, draft.replaceAll('\n', '/')]);
          setDraft('');
        }}
        onStop={running ? () => setStopped(true) : undefined}
      />
      <output aria-label="Sent">{sent.join('|')}</output>
      {stopped && <p>Stopped</p>}
    </div>
  );
}

test('Enter sends the draft and Shift+Enter starts a new line', async () => {
  const screen = await render(<Chat />);
  await screen.getByRole('textbox', { name: 'Message' }).click();
  await userEvent.keyboard('first{Shift>}{Enter}{/Shift}second{Enter}');
  await expect.element(screen.getByText('first/second', { exact: true })).toBeVisible();
  await expect.element(screen.getByRole('textbox', { name: 'Message' })).toHaveValue('');
});

test('keyboard focus shows no ring, on the message box or its buttons (DESIGN.md)', async () => {
  const screen = await render(<Chat running />);
  const input = screen.getByRole('textbox', { name: 'Message' });
  await input.click();
  // Safari's Tab skips buttons unless the person turns that on; Option+Tab reaches them.
  await userEvent.keyboard(server.browser === 'webkit' ? '{Alt>}{Tab}{/Alt}' : '{Tab}');
  const stop = screen.getByRole('button', { name: 'Stop this chat now' }).element() as HTMLElement;
  expect(document.activeElement).toBe(stop);
  expect(stop.matches(':focus-visible')).toBe(true);
  for (const el of [
    input.element(),
    input.element().closest('[data-slot="composer-bar"]') as HTMLElement,
    stop,
  ]) {
    expect(getComputedStyle(el).outlineStyle).toBe('none');
    // A ring is a box-shadow in a colour that shows; a cleared one is fully transparent.
    const colours = getComputedStyle(el).boxShadow.match(/(rgba?|oklch|oklab|lab)\([^)]*\)/g) ?? [];
    expect(colours.filter((c) => c !== 'rgba(0, 0, 0, 0)')).toEqual([]);
  }
  // It gets the hover highlight instead, over its own background.
  expect(getComputedStyle(stop).backgroundImage).toContain('gradient');
});

test('while a turn runs the button stops it, and typing turns it back into Send', async () => {
  const screen = await render(<Chat running />);
  await screen.getByRole('button', { name: 'Stop this chat now' }).click();
  await expect.element(screen.getByText('Stopped')).toBeVisible();

  await screen.getByRole('textbox', { name: 'Message' }).fill('also check the tests');
  await screen.getByRole('button', { name: 'Send' }).click();
  await expect
    .element(screen.getByRole('status', { name: 'Sent' }))
    .toHaveTextContent('also check the tests');
});

const list = () => page.getByRole('listbox', { name: 'Agents' });
const names = () =>
  list()
    .getByRole('option')
    .elements()
    .map((o) => o.textContent);

test('@ at the start of a word lists who can be named, those not in the chat under Bring in', async () => {
  const screen = await render(<Chat />);
  const input = screen.getByRole('textbox', { name: 'Message' });
  await expect.element(input).toHaveAttribute('placeholder', 'Message Ann');
  await input.click();
  await userEvent.keyboard('mail me@');
  expect(list().query()).toBeNull();
  await userEvent.keyboard(' @');
  await expect.element(list()).toBeVisible();
  expect(names()).toEqual(['AnnSupervisor', 'AbeMember', 'Bo']);
  await expect.element(list().getByRole('group', { name: 'Bring in' })).toHaveTextContent('Bring inBo');
  await userEvent.keyboard('a');
  await expect.poll(names).toEqual(['AnnSupervisor', 'AbeMember']);
});

test('Up, Down and Enter pick a name, which goes in as @Name and a space, without sending', async () => {
  const screen = await render(<Chat />);
  const input = screen.getByRole('textbox', { name: 'Message' });
  await input.click();
  await userEvent.keyboard('Look @');
  const lit = () => list().getByRole('option', { selected: true }).element().textContent;
  await expect.poll(lit).toBe('AnnSupervisor');
  await userEvent.keyboard('{ArrowDown}{ArrowDown}');
  expect(lit()).toBe('Bo');
  await userEvent.keyboard('{ArrowDown}');
  expect(lit()).toBe('AnnSupervisor');
  await userEvent.keyboard('{ArrowUp}{Enter}');
  await expect.element(input).toHaveValue('Look @Bo ');
  expect(list().query()).toBeNull();
  await userEvent.keyboard('now');
  await expect.element(input).toHaveValue('Look @Bo now');
  expect(screen.getByRole('status', { name: 'Sent' }).element().textContent).toBe('');
});

test('Escape closes the list, which stays shut for that @ until another is typed', async () => {
  const screen = await render(<Chat />);
  await screen.getByRole('textbox', { name: 'Message' }).click();
  await userEvent.keyboard('@');
  await expect.element(list()).toBeVisible();
  await userEvent.keyboard('{Escape}');
  expect(list().query()).toBeNull();
  await userEvent.keyboard('A');
  expect(list().query()).toBeNull();
  await userEvent.keyboard(' and @');
  await expect.element(list()).toBeVisible();
});

test('a pointer click on a name picks it, in place of the part typed, and the keyboard stays in the box', async () => {
  const screen = await render(<Chat />);
  const input = screen.getByRole('textbox', { name: 'Message' });
  await input.click();
  await userEvent.keyboard('Ask @ab');
  await list().getByRole('option', { name: /Abe/ }).click();
  await expect.element(input).toHaveValue('Ask @Abe ');
  await expect.element(input).toHaveFocus();
  expect(list().query()).toBeNull();
});

test('only a name picked from the list is blue under the text; one typed out is plain', async () => {
  const screen = await render(<Chat />);
  await screen.getByRole('textbox', { name: 'Message' }).click();
  await userEvent.keyboard('@ann and @bo, ask @ab{Enter}now');
  const layer = document.querySelector('.composer-mirror') as HTMLElement;
  // The box's text in one piece (a span would move the letters after it), and only the picked name is blue.
  expect([...layer.childNodes].map((n) => n.textContent)).toEqual(['@ann and @bo, ask @Abe now ']);
  const blue = [...(CSS.highlights.get('composer-mention') ?? [])] as Range[];
  expect(blue.map((r) => r.toString())).toEqual(['@Abe']);
});
