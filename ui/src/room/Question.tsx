import { Check } from 'lucide-react';
import { useState } from 'react';
import { Button } from '@/components/ui/button';
import type { Failure, Question } from '../api';
import { Composer } from '../components/Composer';
import { Face } from '../components/Face';
import { Markdown } from '../components/Markdown';
import { FailureNotice } from '../components/Notice';
import { StateMark } from '../components/StateMark';
import { clock, type People } from './Room';

interface Props {
  question: Question;
  people: People;
  /** Sends the person's answer: it fails with the service's refusal. */
  onAnswer: (id: number, pick?: string, words?: string) => Promise<unknown>;
  onReference: (ref: string) => void;
  /** Whether a line runs down from its picture to the next card. */
  joined?: boolean;
}

/**
 * An agent's question to the person, as a card of its own: the question, its choices and a box for their own words.
 * A choice answers at once, with any words typed; the words alone answer with Send. Answered, it goes quiet.
 */
export function QuestionCard({ question: q, people, onAnswer, onReference, joined }: Props) {
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
          <Answer question={q} onAnswer={onAnswer} onReference={onReference} />
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

function Answer({ question: q, onAnswer, onReference }: Omit<Props, 'people'>) {
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

/** An answered question as one quiet line in the chat: who asked what, and the answer. */
export function AnsweredLine({ question: q, people }: { question: Question; people: People }) {
  const answer = [q.pick, q.words].filter(Boolean).join(' · ');
  return (
    <div className="room-asked-line">
      <Face agent={people.face(q.agent)} size={20} />
      <span>
        <b>{q.name}</b> asked “{q.question.split('\n')[0]}” · You: {answer}
      </span>
    </div>
  );
}
