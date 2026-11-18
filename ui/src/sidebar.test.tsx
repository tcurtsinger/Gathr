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

test("the keyboard shows a row's More as it reaches it", async () => {
  await sidebar();
  await page.getByRole('button', { name: 'Made-up open chat', exact: true }).click();
  await userEvent.unhover(page.getByRole('button', { name: 'Made-up open chat', exact: true }));
  await userEvent.keyboard('{Tab}');
  await expect.element(more('Made-up open chat')).toHaveFocus();
  expect(opacity('Made-up open chat'), 'its More, focused by the keyboard').toBe(1);
});
