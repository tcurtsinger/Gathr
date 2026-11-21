import { SettingChoice } from './SettingChoice';

/** The owner's list (room task 41), and No limit: no Off, which would bring back what the limit fixes. */
const choices = [
  ...[5, 10, 25, 50].map((n) => ({ value: n, label: `${n} hand-offs${n === 50 ? ' (default)' : ''}` })),
  { value: null, label: 'No limit' },
];

/** Settings > Teamwork: how many messages between agents in a row a team chat takes before it pauses. */
export function HandoffLimit() {
  return (
    <SettingChoice
      field="handoff_limit"
      label="Pause after"
      line="A chat pauses after this many agent messages in a row."
      choices={choices}
      // A limit set another way (the API takes 1 to 1000) still shows as it is.
      other={(n) => `${n} hand-offs`}
    />
  );
}
