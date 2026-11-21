import { Dropdown } from '../components/Dropdown';
import { Row } from './Row';
import { useSettings } from './saved';

const NONE = 'none';

interface Props {
  /** The setting it shows and saves: a number, or null. */
  field: 'handoff_limit' | 'auto_compact';
  label: string;
  line: string;
  /** Each choice, null for the setting's none. */
  choices: { value: number | null; label: string }[];
  /** A value set another way (through the API) that isn't one of the choices, as it reads. */
  other: (value: number) => string;
}

/** A setting in Settings > Teamwork picked from a list, saved when picked. */
export function SettingChoice({ field, label, line, choices, other }: Props) {
  const { saved, save } = useSettings();
  if (!saved) return null;
  const value = saved[field] ?? null;

  return (
    <Row name={<label htmlFor={field}>{label}</label>} line={line}>
      <Dropdown
        id={field}
        value={String(value ?? NONE)}
        onChange={(v) => save({ [field]: v === NONE ? null : Number(v) })}
        choices={[
          ...(value !== null && !choices.some((c) => c.value === value)
            ? [{ value: String(value), label: other(value) }]
            : []),
          ...choices.map((c) => ({ value: String(c.value ?? NONE), label: c.label })),
        ]}
      />
    </Row>
  );
}
