import { type ElicitationField, ElicitationForm } from '@/components/assistant-ui/elements/elicitation-form';
import { OptionList } from '@/components/assistant-ui/elements/option-list';
import type { PersonTask } from '../api';
import { Switch } from '../components/Switch';
import { type Draft, options } from './model';

/**
 * Picks stage the answer; nothing is confirmed here. A pick also takes optional notes. Discuss, on an
 * agent's question, sends a message in place of an answer: Gathr's own can only be answered.
 */
export function TaskAnswer({
  task,
  draft,
  onDraft,
  name,
}: {
  task: PersonTask;
  draft: Draft;
  onDraft: (d: Draft) => void;
  name: (agent: string | null) => string;
}) {
  const text = task.form === 'text';
  const field: ElicitationField = draft.discuss
    ? {
        name: 'notes',
        label: 'Your message',
        kind: 'text',
        value: draft.notes,
        required: true,
        multiline: true,
      }
    : text
      ? {
          name: 'answer',
          label: 'Your answer',
          kind: 'text',
          value: draft.values[0] ?? '',
          required: true,
          multiline: true,
        }
      : { name: 'notes', label: 'Notes (optional)', kind: 'text', value: draft.notes, multiline: true };
  const label = `discuss-${task.number}`;
  return (
    <div className="tasks-answer">
      {!text && !draft.discuss && (
        <OptionList
          presentation="inline"
          aria-label="Your answer"
          options={options(task, name)}
          selectionMode={task.form === 'choices' || task.form === 'done' ? 'multiple' : 'single'}
          value={draft.values}
          onValueChange={(values) => onDraft({ ...draft, values })}
        />
      )}
      <ElicitationForm
        presentation="inline"
        server=""
        message=""
        state="request"
        aria-label="Task answer"
        fields={[field]}
        onFieldChange={(name, value) =>
          onDraft(name === 'notes' ? { ...draft, notes: value } : { ...draft, values: [value] })
        }
      />
      {task.giver !== null && (
        <div className="tasks-discuss">
          <span id={label}>Discuss</span>
          <Switch
            on={!!draft.discuss}
            labelledBy={label}
            onChange={(discuss) => onDraft({ ...draft, discuss })}
          />
        </div>
      )}
    </div>
  );
}
