import { SettingChoice } from './SettingChoice';

/** The owner's list (compaction plan, K3). */
const choices = [
  ...[200, 300, 400, 500].map((k) => ({ value: k * 1000, label: `${k}K${k === 500 ? ' (default)' : ''}` })),
  { value: null, label: "Engine's own" },
];

/** Settings > Teamwork: the tokens at which an agent's engine compacts its conversation by itself. */
export function AutoCompact() {
  return (
    <SettingChoice
      field="auto_compact"
      label="Auto-compact"
      line="How full a conversation gets before it is compacted."
      choices={choices}
      other={(n) => `${Math.round(n / 1000)}K`}
    />
  );
}
