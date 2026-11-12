import { SettingChoice } from './SettingChoice';

/** The owner's list (compaction plan, K3). */
const choices = [
  ...[200, 300, 400, 500].map((k) => ({ value: k * 1000, label: `${k}K${k === 500 ? ' (default)' : ''}` })),
  { value: null, label: "Engine's own" },
];

/** Settings > General: the tokens at which an agent's engine compacts its conversation by itself. */
export function AutoCompact({ onReference }: { onReference: (ref: string) => void }) {
  return (
    <SettingChoice
      field="auto_compact"
      label="Auto-compact"
      help={{
        label: 'About auto-compact',
        text: (
          <p>
            How full an agent's conversation gets before its engine compacts it, never past 80% of its model's
            window. Applies from each engine's next start.
          </p>
        ),
      }}
      choices={choices}
      other={(n) => `${Math.round(n / 1000)}K`}
      onReference={onReference}
    />
  );
}
