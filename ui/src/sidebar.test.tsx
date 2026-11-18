// A chat's More shows only while the pointer is on its row or the keyboard is in it, the open chat's row
// too (C70). Measured as drawn, with the app's styles.
import { expect, test } from 'vitest';
import { page, userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import { Sidebar } from './Sidebar';

const none = () => {};

async function sidebar() {
  await render(
    <Sidebar
      chats={[
        { id: 'c1', title: 'Made-up open chat' },
        { id: 'c2', title: 'Made-up other chat' },
      ]}
      archived={[]}
      projects={[]}
      selected="c1"
      onSelect={none}
      onNew={none}
      onProjectSettings={none}
      onDeleteProject={none}
      onArchive={none}
      onDelete={none}
      onRestore={none}
    />,
  );
}

const more = (title: string) => page.getByRole('button', { name: `More for ${title}` });
const opacity = (title: string) => Number(getComputedStyle(more(title).element()).opacity);

test("the open chat's More is hidden until the pointer is on its row", async () => {
  await sidebar();
  await expect.element(more('Made-up open chat')).toBeInTheDocument();
  expect(opacity('Made-up open chat'), "the open chat's More, with the pointer away").toBe(0);

  await userEvent.hover(page.getByRole('button', { name: 'Made-up open chat', exact: true }));
  await expect
    .poll(() => opacity('Made-up open chat'), { message: 'its More, the pointer on the row' })
    .toBe(1);
});

// Focus is set by hand, not with Tab: WebKit's Tab passes over every button unless the Mac is set to visit them,
// so a Tab there says nothing about this row.
test('focus in a row shows its More, which takes focus though it had no room', async () => {
  await sidebar();
  const row = page.getByRole('button', { name: 'Made-up open chat', exact: true });
  // The pointer rests where the test before left it.
  await userEvent.unhover(row);
  (row.element() as HTMLElement).focus();
  await expect.poll(() => opacity('Made-up open chat'), { message: 'its More, focus on the row' }).toBe(1);

  (row.element() as HTMLElement).blur();
  await expect.poll(() => opacity('Made-up open chat'), { message: 'its More, focus gone' }).toBe(0);
  (more('Made-up open chat').element() as HTMLElement).focus();
  await expect.element(more('Made-up open chat')).toHaveFocus();
  expect(opacity('Made-up open chat'), 'its More, focused').toBe(1);
});
