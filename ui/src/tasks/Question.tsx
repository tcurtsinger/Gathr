import { ArrowUp, CircleHelp } from 'lucide-react';
import { type FormEvent, type KeyboardEvent, useId, useLayoutEffect, useRef, useState } from 'react';
import type { PersonTask } from '../api';
import { Switch } from '../components/Switch';
import { type Draft, gathrs, options, type Refusal, ready } from './model';
import './question.css';

interface Props {
  task: PersonTask;
  draft: Draft;
  onDraft: (draft: Draft) => void;
  name: (agent: string | null) => string;
  /** Sends this question's answer, or its Discuss message: none once the service took it, else why not. */
  onSend: (draft: Draft) => Promise<Refusal | null>;
  onReference: (ref: string) => void;
}

/**
 * The one question block (0065): what was asked, answered in place, each question by itself. Gathr's own question
 * takes a choice and nothing else, so a pick answers it at once; an agent's has a field, so a pick marks the choice
 * and Send sends it with its notes. Discuss, right under the question, swaps the choices and notes for a message to
 * the asker; each is kept while the other shows. A refused Send stays here with why and its reference.
 */
export function Question({ task, draft, onDraft, name, onSend, onReference }: Props) {
  const [sending, setSending] = useState(false);
  const [refusal, setRefusal] = useState<Refusal | null>(null);
  const label = useId();
  const open = task.state === 'ready';
  const own = gathrs(task);
  const talking = !own && !!draft.discuss;
  const words = task.form === 'text';
  const many = task.form === 'choices' || task.form === 'done';

  const send = async (sent: Draft) => {
    if (sending || !ready(task, sent)) return;
    setSending(true);
    setRefusal(await onSend(sent));
    setSending(false);
  };
  const pick = (id: string) => {
    const on = draft.values.includes(id);
    const values = many
      ? on
        ? draft.values.filter((v) => v !== id)
        : [...draft.values, id]
      : on
        ? []
        : [id];
    const next = { ...draft, values };
    onDraft(next);
    if (own && values.length) send(next);
  };

  const place = talking ? `Message to ${name(task.giver)}` : words ? 'Answer' : 'Notes';
  const value = talking ? (draft.message ?? '') : words ? (draft.values[0] ?? '') : draft.notes;
  const type = (text: string) =>
    onDraft(
      talking
        ? { ...draft, message: text }
        : words
          ? { ...draft, values: [text] }
          : { ...draft, notes: text },
    );

  return (
    <div className="question" data-open={open || undefined}>
      <div className="question-head">
        <CircleHelp aria-hidden="true" />
        <span>{task.what}</span>
        {open ? (
          <span className="question-waiting">Waiting</span>
        ) : (
          task.state === 'held' && <span className="question-word">Discussing</span>
        )}
      </div>
      {open && (
        <>
          {!own && (
            <div className="question-discuss">
              <Switch
                on={talking}
                labelledBy={label}
                onChange={(discuss) => onDraft({ ...draft, discuss })}
              />
              <span id={label}>Discuss</span>
            </div>
          )}
          {!words && !talking && (
            <fieldset className="question-choices" aria-label="Answer">
              {options(task, name).map((o) => (
                <button
                  type="button"
                  key={o.id}
                  className="question-choice"
                  aria-pressed={draft.values.includes(o.id)}
                  disabled={sending}
                  onClick={() => pick(o.id)}
                >
                  <span className="question-mark" data-many={many || undefined} />
                  <span className="question-choice-text">
                    <span className="question-choice-label">{o.label}</span>
                    {o.description && <small>{o.description}</small>}
                  </span>
                </button>
              ))}
            </fieldset>
          )}
          {!own && (
            <Field
              place={place}
              value={value}
              onType={type}
              ready={!sending && ready(task, draft)}
              onSend={() => send(draft)}
            />
          )}
          {refusal && <Refused word="Not sent" refusal={refusal} onReference={onReference} />}
        </>
      )}
    </div>
  );
}

/** What the service refused, in place: "Not sent", why, and its reference. */
export function Refused(props: {
  word: string;
  refusal: Pick<Refusal, 'why' | 'ref'>;
  onReference: (ref: string) => void;
}) {
  const { word, refusal, onReference } = props;
  return (
    <p className="question-refused">
      <span className="bad">{word}</span>
      <span aria-hidden="true">·</span>
      <span>{refusal.why}</span>
      {refusal.ref && (
        <button type="button" className="question-ref" onClick={() => onReference(refusal.ref as string)}>
          {refusal.ref}
        </button>
      )}
    </p>
  );
}

/**
 * The one field for words to the room (the question block's notes, answer or Discuss message, the plan's Send
 * back), growing with what is typed; Enter sends. `arrow: false` leaves its send to buttons beside it.
 */
export function Field(props: {
  place: string;
  value: string;
  onType: (text: string) => void;
  ready: boolean;
  onSend: () => void;
  arrow?: boolean;
}) {
  const { place, value, onType, ready, onSend, arrow = true } = props;
  const box = useRef<HTMLTextAreaElement>(null);
  // biome-ignore lint/correctness/useExhaustiveDependencies: it grows with what it holds
  useLayoutEffect(() => {
    const el = box.current;
    if (!el) return;
    el.style.height = 'auto';
    el.style.height = `${el.scrollHeight + 2}px`;
  }, [value]);
  const submit = (e: FormEvent) => {
    e.preventDefault();
    onSend();
  };
  const keys = (e: KeyboardEvent) => {
    if (e.key !== 'Enter' || e.shiftKey) return;
    e.preventDefault();
    onSend();
  };
  return (
    <form className="question-field" onSubmit={submit}>
      <textarea
        ref={box}
        rows={1}
        aria-label={place}
        placeholder={place}
        value={value}
        onChange={(e) => onType(e.target.value)}
        onKeyDown={keys}
      />
      {arrow && (
        <button type="submit" className="question-send" aria-label="Send" disabled={!ready}>
          <ArrowUp className="question-send-icon" aria-hidden="true" />
        </button>
      )}
    </form>
  );
}
