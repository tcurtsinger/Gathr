import { expect, test } from 'vitest';
import { render } from 'vitest-browser-react';
import { Workspace } from './Workspace';

test('when the open agent has left the chat its tab goes, and Tasks shows in its place', async () => {
  const none = () => {};
  const shown = (agent?: { name: string; panel: string }) => (
    <Workspace
      waiting={false}
      files={[]}
      agent={agent}
      selected="agent"
      onSelect={none}
      onCloseFile={none}
      onCloseAgent={none}
      onClose={none}
      tasks="Made-up tasks"
    />
  );
  const screen = await render(shown({ name: 'Made-up Member', panel: 'Made-up panel' }));
  await expect
    .element(screen.getByRole('tab', { name: 'Made-up Member' }))
    .toHaveAttribute('aria-selected', 'true');
  await expect.element(screen.getByRole('tabpanel')).toHaveTextContent('Made-up panel');

  await screen.rerender(shown());
  await expect.element(screen.getByRole('tab', { name: 'Made-up Member' })).not.toBeInTheDocument();
  await expect.element(screen.getByRole('tab', { name: 'Tasks' })).toHaveAttribute('aria-selected', 'true');
  await expect.element(screen.getByRole('tabpanel')).toHaveTextContent('Made-up tasks');
});
