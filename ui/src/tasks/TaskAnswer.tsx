import { type ElicitationField, ElicitationForm } from '@/components/assistant-ui/elements/elicitation-form';
import { OptionList } from '@/components/assistant-ui/elements/option-list';
import type { Draft, PersonTask } from './model';

/** Picks stage the answer; nothing is confirmed here. A pick also takes optional notes. */
export function TaskAnswer({
  task,
  draft,
  onDraft,
}: {
  task: PersonTask;
  draft: Draft;
  onDraft: (d: Draft) => void;
}) {
  const text = task.answer === 'text';
  const field: ElicitationField = text
    ? {
        name: 'answer',
        label: 'Your answer',
        kind: 'text',
        value: draft.values[0] ?? '',
        required: true,
        multiline: true,
      }
    : { name: 'notes', label: 'Notes (optional)', kind: 'text', value: draft.notes, multiline: true };
  return (
    <div className="tasks-answer">
      {!text && (
        <OptionList
          presentation="inline"
          aria-label="Your answer"
          options={task.options}
          selectionMode={task.answer === 'many' ? 'multiple' : 'single'}
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
    </div>
  );
}
