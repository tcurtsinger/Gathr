import { useEffect, useRef, useState } from 'react';
import { expect, test } from 'vitest';
import { userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import { ColourPicker } from './ColourPicker';

/** The picker inside a modal dialog, as Settings holds it, opened by a button under its anchor. */
function InDialog({ start }: { start: string }) {
  const dialog = useRef<HTMLDialogElement>(null);
  const anchor = useRef<HTMLSpanElement>(null);
  const [value, setValue] = useState(start);
  const [closed, setClosed] = useState(false);
  useEffect(() => dialog.current?.showModal(), []);
  return (
    <>
      <dialog ref={dialog} onCancel={() => setClosed(true)}>
        <span ref={anchor}>
          <button type="button" popoverTarget="made-up-picker">
            Open
          </button>
        </span>
        <ColourPicker id="made-up-picker" label="Made-up" anchor={anchor} value={value} onChange={setValue} />
        <p>Now {value}</p>
      </dialog>
      <p>{closed ? 'Dialog closed' : 'Dialog open'}</p>
    </>
  );
}

async function opened(start: string) {
  const screen = await render(<InDialog start={start} />);
  await screen.getByRole('button', { name: 'Open' }).click();
  await expect.element(screen.getByRole('group', { name: 'Made-up' })).toBeVisible();
  return screen;
}

test('arrow keys move the square and the hue bar, and each step is the colour now', async () => {
  // #7FB5A8: hue 166, saturation 30%, brightness 71%.
  const screen = await opened('#7FB5A8');
  const square = screen.getByRole('slider', { name: 'Colour' });
  const hue = screen.getByRole('slider', { name: 'Hue' });
  await expect.element(square).toHaveAttribute('aria-valuenow', '30');
  await expect.element(hue).toHaveAttribute('aria-valuenow', '166');

  square.element().focus();
  await userEvent.keyboard('{Shift>}{ArrowRight}{/Shift}');
  await expect.element(square).toHaveAttribute('aria-valuenow', '40');
  await userEvent.keyboard('{ArrowDown}');
  await expect.element(square).toHaveAttribute('aria-valuetext', 'Saturation 40%, brightness 70%');
  const moved = screen.getByText(/^Now #/).element().textContent;
  expect(moved).not.toBe('Now #7FB5A8');

  hue.element().focus();
  await userEvent.keyboard('{ArrowLeft}{ArrowLeft}');
  await expect.element(hue).toHaveAttribute('aria-valuenow', '164');
  await userEvent.keyboard('{End}');
  await expect.element(hue).toHaveAttribute('aria-valuenow', '359');
  expect(screen.getByText(/^Now #/).element().textContent).not.toBe(moved);
});

test('a typed hex sets the colour, three digits once it is done; one that is not a colour changes nothing', async () => {
  const screen = await opened('#7FB5A8');
  const hex = screen.getByRole('textbox', { name: 'Hex' });
  await expect.element(hex).toHaveValue('7FB5A8');

  await hex.fill('e6a4e');
  await expect.element(screen.getByText('Now #7FB5A8')).toBeVisible();
  await hex.fill('#e6a4e0');
  await expect.element(screen.getByText('Now #E6A4E0')).toBeVisible();

  await hex.fill('zz12');
  await userEvent.keyboard('{Enter}');
  await expect.element(screen.getByText('Now #E6A4E0')).toBeVisible();
  await expect.element(hex).toHaveValue('E6A4E0');

  await hex.fill('abc');
  await expect.element(screen.getByText('Now #E6A4E0')).toBeVisible();
  await userEvent.keyboard('{Enter}');
  await expect.element(screen.getByText('Now #AABBCC')).toBeVisible();
  await expect.element(hex).toHaveValue('AABBCC');
  await expect.element(screen.getByText('Dialog open')).toBeVisible();
});

test('a grey keeps the hue bar where it was', async () => {
  const screen = await opened('#7FB5A8');
  const hue = screen.getByRole('slider', { name: 'Hue' });
  await screen.getByRole('textbox', { name: 'Hex' }).fill('808080');
  await expect.element(screen.getByText('Now #808080')).toBeVisible();
  await expect.element(hue).toHaveAttribute('aria-valuenow', '166');
  await expect.element(screen.getByRole('slider', { name: 'Colour' })).toHaveAttribute('aria-valuenow', '0');
});

test('Escape closes the picker, not the dialog it sits in', async () => {
  const screen = await opened('#7FB5A8');
  screen.getByRole('slider', { name: 'Hue' }).element().focus();
  await userEvent.keyboard('{Escape}');
  await expect.element(screen.getByRole('group', { name: 'Made-up' })).not.toBeInTheDocument();
  await expect.element(screen.getByText('Dialog open')).toBeVisible();
  await userEvent.keyboard('{Escape}');
  await expect.element(screen.getByText('Dialog closed')).toBeVisible();
});
