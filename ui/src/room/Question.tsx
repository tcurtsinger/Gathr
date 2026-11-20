import { Check } from 'lucide-react';
import { Popover } from 'radix-ui';
import { useState } from 'react';
import { Button } from '@/components/ui/button';
import type { Failure, Question } from '../api';
import { Composer } from '../components/Composer';
import { Face } from '../components/Face';
import { IconButton } from '../components/IconButton';
import { Markdown } from '../components/Markdown';
import { FailureNotice } from '../components/Notice';
import { StateMark } from '../components/StateMark';
import { clock, type People } from './Room';
import '../components/popover-panel.css';

/** Sends the person's answer: it fails with the service's refusal. */
type OnAnswer = (id: number, pick?: string, words?: string) => Promise<unknown>;

/**
 * An agent's question to the person, as a card of its own where it was asked: the question and, while it is open,
 * Answer, which opens the questions panel on it. Answered, the answer sits under it.
 */
export function QuestionCard(props: {
  question: Question;
  people: People;
  onOpen: (id: number) => void;
  /** Whether a line runs down from its picture to the next card. */
  joined?: boolean;
}) {
  const { question: q, people, onOpen, joined } = props;
  const open = !q.answered;
  return (
    <div className="room-item room-turn">
      <span className="room-who">
        <Face agent={people.face(q.agent)} size={28} mark={{ state: open ? 'needs' : 'free' }} />
      </span>
      {joined && <span className="room-rule" aria-hidden="true" />}
      <article className="room-card room-question" data-open={open || undefined} aria-label={q.name}>
        <header className="room-head">
          <b>{q.name}</b>
          {people.lead(q.agent) && <span className="room-tag">Lead</span>}
          <span className="room-grow" />
          {open ? (
            <span className="room-state live">
              <StateMark state="needs" size={12} />
              Needs you
            </span>
          ) : (
            <span className="room-state">
              <Check aria-hidden="true" />
              Answered
            </span>
          )}
        </header>
        <Markdown className="room-body" text={q.question} names={people.names} />
        {open ? (
          <div className="room-asks">
            <Button type="button" variant="outline" size="sm" data-answer={q.id} onClick={() => onOpen(q.id)}>
              Answer
            </Button>
          </div>
        ) : (
          <div className="room-answer">
            <div className="room-line">
              <b>You</b> <span className="room-time">{clock(q.answered)}</span>
            </div>
            <p>
              {q.pick && <b>{q.pick}</b>}
              {q.pick && q.words && ' · '}
              {q.words}
            </p>
          </div>
        )}
      </article>
    </div>
  );
}

/**
 * The top line's Questions button and its panel: the chat's open questions, each with its choices and a box for the
 * person's own words. The button's dot is filled while one is open. `at`: undefined while the panel is closed, else
 * the question it opens on, or null for none.
 */
export function Questions(props: {
  questions: Question[];
  people: People;
  at: number | null | undefined;
  onOpen: (at: number | null | undefined) => void;
  onAnswer: OnAnswer;
  onReference: (ref: string) => void;
}) {
  const { people, at, onOpen, onAnswer, onReference } = props;
  const open = props.questions.filter((q) => !q.answered);
  return (
    <Popover.Root open={at !== undefined} onOpenChange={(o) => onOpen(o ? null : undefined)}>
      <Popover.Trigger asChild>
        <IconButton aria-label="Questions" className="head-button">
          <svg
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth={2}
            strokeLinecap="round"
            strokeLinejoin="round"
            aria-hidden="true"
          >
            <path d="M13 3H7a4 4 0 0 0-4 4v10a4 4 0 0 0 4 4h10a4 4 0 0 0 4-4v-6" />
            <circle
              cx="18.5"
              cy="5.5"
              r="2.5"
              className="questions-dot"
              data-open={open.length > 0 || undefined}
            />
            <path d="M7.5 13h7M7.5 17h4" />
          </svg>
        </IconButton>
      </Popover.Trigger>
      <Popover.Portal>
        <Popover.Content
          className="popover-panel questions"
          side="bottom"
          align="end"
          sideOffset={8}
          collisionPadding={16}
          role="dialog"
          aria-label="Questions"
          onOpenAutoFocus={(e) => {
            if (show(at)) e.preventDefault();
          }}
          // Radix dismisses on the click after a card's Answer has set the question. That click isn't a click away:
          // it moves the open panel to its question, even when that is already the one set.
          onInteractOutside={(e) => {
            const answer = e.target instanceof HTMLElement && e.target.closest<HTMLElement>('[data-answer]');
            if (!answer) return;
            e.preventDefault();
            show(Number(answer.dataset.answer));
          }}
        >
          <h3>Questions</h3>
          {open.length === 0 && <p className="questions-none">No open questions</p>}
          {open.map((q) => (
            <section key={q.id} className="questions-item" data-question={q.id} aria-label={q.name}>
              <div className="room-line">
                <Face agent={people.face(q.agent)} size={20} />
                <b>{q.name}</b> <span className="room-time">{clock(q.asked)}</span>
              </div>
              <Markdown className="room-said" text={q.question} names={people.names} />
              <Answer question={q} onAnswer={onAnswer} onReference={onReference} />
            </section>
          ))}
        </Popover.Content>
      </Popover.Portal>
    </Popover.Root>
  );
}

/** Scrolls the panel to a question and puts the cursor in its answer; null while it isn't shown. */
function show(at: number | null | undefined) {
  const item = document.querySelector(`[data-question="${at}"]`);
  item?.scrollIntoView({ block: 'nearest' });
  item?.querySelector<HTMLElement>('button, textarea')?.focus();
  return item;
}

/** A choice answers at once, with any words typed; the words alone answer with Send. */
function Answer(props: { question: Question; onAnswer: OnAnswer; onReference: (ref: string) => void }) {
  const { question: q, onAnswer, onReference } = props;
  const [words, setWords] = useState('');
  const [sending, setSending] = useState(false);
  const [refused, setRefused] = useState<Failure>();
  const send = (pick?: string) => {
    if (sending) return;
    setSending(true);
    onAnswer(q.id, pick, words).then(
      () => setRefused(undefined),
      (f: Failure) => {
        setRefused(f);
        setSending(false);
      },
    );
  };
  return (
    <div className="room-asks">
      {q.choices.length > 0 && (
        <div className="room-choices">
          {q.choices.map((c) => (
            <Button
              key={c.label}
              type="button"
              variant="outline"
              className="h-auto min-h-8 py-1 whitespace-normal text-left"
              disabled={sending}
              onClick={() => send(c.label)}
            >
              <span>
                {c.label}
                {c.detail && <small className="room-choice-detail">{c.detail}</small>}
              </span>
            </Button>
          ))}
        </div>
      )}
      <Composer
        draft={words}
        onDraft={(text) => setWords(text)}
        onSend={() => send()}
        hint="Your answer"
        label="Answer"
      />
      {refused && <FailureNotice failure={refused} onReference={onReference} />}
    </div>
  );
}
