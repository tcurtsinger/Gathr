import { SettingChoice } from './SettingChoice';

/** The owner's list (room task 41), and No limit: no Off, which would bring back what the limit fixes. */
const choices = [
  ...[5, 10, 25, 50].map((n) => ({ value: n, label: `${n} hand-offs${n === 50 ? ' (default)' : ''}` })),
  { value: null, label: 'No limit' },
];

/**
 * Settings > General: how many messages between agents in a row a team chat takes before it pauses
 * (plans/agent-messages.md).
 */
export function HandoffLimit({ onReference }: { onReference: (ref: string) => void }) {
  return (
    <SettingChoice
      field="handoff_limit"
      label="Pause after"
      help={{
        label: 'About hand-offs',
        text: (
          <p>
            Messages between agents in a row, with nothing from you and no task finished between them. Past
            the limit the chat pauses until you resume it.
          </p>
        ),
      }}
      choices={choices}
      // A limit set another way (the API takes 1 to 1000) still shows as it is.
      other={(n) => `${n} hand-offs`}
      onReference={onReference}
    />
  );
}
