import { Check } from 'lucide-react';
import { Popover } from 'radix-ui';
import { useState } from 'react';
import { Button } from '@/components/ui/button';
import type { Failure, Question, Turn } from '../api';
import { type Approval, asked, titleOf } from '../components/Approval';
import { Composer } from '../components/Composer';
import { useCard } from '../components/Corner';
import { Face } from '../components/Face';
import { PathTiles } from '../components/FileTile';
import { split } from '../components/files';
import { IconButton } from '../components/IconButton';
import { clock, LeadMark, type People, Words } from './Room';
import '../components/popover-panel.css';

/** Whether the question waits for the person's answer: not answered, nor closed when its chat was archived. */
export const waits = (q: Question) => !q.answered && !q.closed;

/** Sends the person's answer: it fails with the service's refusal. */
export type OnAnswer = (
  question: Question,
  pick?: string,
  words?: string,
  files?: readonly File[],
) => Promise<unknown>;

/**
 * An agent's question to the person, as a card of its own where it was asked: the question and, while it is open, its
 * choices as rows to pick and Answer (none in an archived chat, which reads only). Its blue frame says it waits on the
 * person. Answered, the answer sits under it; the asker's name is under the card.
 */
export function QuestionCard(props: { question: Question; people: People; onAnswer?: OnAnswer }) {
  const { question: q, people, onAnswer } = props;
  const open = waits(q);
  const answer = split(q.words ?? '');
  return (
    <div className="room-item room-turn">
      <span className="room-who">
        <Face agent={people.face(q.agent)} size={28} />
        <LeadMark people={people} agent={q.agent} />
      </span>
      <article
        className="room-card room-question"
        data-open={open || undefined}
        data-asked={q.id}
        aria-label={open ? `${q.name}, needs you` : q.name}
      >
        <Words className="room-body" text={q.question} chat={q.chat} names={people.names} agent={q.agent} />
        {open ? (
          onAnswer && <Choose question={q} onAnswer={onAnswer} />
        ) : q.closed ? (
          <p className="room-closed">Closed when archived</p>
        ) : (
          <div className="room-answer">
            <div className="room-line">
              <b>You</b> <span className="room-time">{clock(q.answered)}</span>
            </div>
            <p>
              {q.pick && <b>{q.pick}</b>}
              {q.pick && answer.words && ' · '}
              {answer.words}
            </p>
            {answer.files.length > 0 && <PathTiles chat={q.chat} paths={answer.files} />}
          </div>
        )}
      </article>
      <div className="room-bar">
        <b className="room-who-name">{q.name}</b>
        {q.closed ? (
          <span className="room-state">Closed</span>
        ) : (
          !open && (
            <span className="room-state">
              <Check aria-hidden="true" />
              Answered
            </span>
          )
        )}
      </div>
    </div>
  );
}

/** The words the person writes instead of a choice: the last row, "Something else…". */
const OWN = '\u0000own';

/**
 * A question's answer on its card: a row per choice and "Something else…", then Answer. Something else opens a box
 * for the person's own words, which sends them. A question with no choices has the box alone.
 */
function Choose({ question: q, onAnswer }: { question: Question; onAnswer: OnAnswer }) {
  const [picked, setPicked] = useState<string>(q.choices.length ? '' : OWN);
  const [words, setWords] = useState('');
  const [files, setFiles] = useState<File[]>([]);
  const [sending, setSending] = useState(false);
  const [refused, setRefused] = useState<Failure>();
  useCard(refused);
  const send = () => {
    if (sending) return;
    setSending(true);
    const own = picked === OWN;
    onAnswer(q, own ? undefined : picked, own ? words : undefined, own ? files : undefined).then(
      () => setRefused(undefined),
      (f: Failure) => {
        setRefused(f);
        setSending(false);
      },
    );
  };
  const rows = [...q.choices, ...(q.choices.length ? [{ label: OWN, detail: null }] : [])];
  return (
    <div className="room-choose">
      {rows.length > 0 && (
        <div className="room-rows">
          {rows.map((c) => (
            <button
              key={c.label}
              type="button"
              aria-pressed={picked === c.label}
              className="room-row"
              data-own={c.label === OWN || undefined}
              disabled={sending}
              onClick={() => setPicked(c.label)}
            >
              <i aria-hidden="true" />
              <span>{c.label === OWN ? 'Something else…' : c.label}</span>
              {c.detail && <small>{c.detail}</small>}
            </button>
          ))}
        </div>
      )}
      {picked === OWN ? (
        <Composer
          draft={words}
          onDraft={(text) => setWords(text)}
          files={files}
          onFiles={setFiles}
          onSend={send}
          hint="Your answer"
          label="Answer"
        />
      ) : (
        <div className="room-choose-foot">
          <Button
            type="button"
            size="sm"
            className="rounded-full"
            disabled={!picked || sending}
            onClick={send}
          >
            Answer
          </Button>
        </div>
      )}
    </div>
  );
}

/**
 * The top line's Needs you button and its panel: what waits on the person, the chat's open questions then its
 * approvals, one row each. A row closes the panel and goes to its card in the chat, where it is answered. The button's
 * dot is lit while one waits.
 */
export function NeedsYou(props: {
  questions: Question[];
  approvals: Approval[];
  turns: Record<string, Turn>;
  people: People;
}) {
  const { people } = props;
  const [open, setOpen] = useState(false);
  const ask = props.questions.filter(waits);
  const rows = [
    ...ask.map((q) => ({
      key: `q${q.id}`,
      agent: q.agent,
      card: `[data-asked="${q.id}"]`,
      time: clock(q.asked),
      what: null,
      text: firstLine(q.question),
    })),
    ...props.approvals.map((a) => ({
      key: `a${a.request}`,
      agent: props.turns[a.turn]?.agent ?? '',
      card: `[data-approval="${a.request}"]`,
      time: null,
      what: titleOf(a.tool),
      text: asked(a).command ?? '',
    })),
  ];
  const go = (card: string) => {
    setOpen(false);
    const el = document.querySelector<HTMLElement>(card);
    if (!el) return;
    el.scrollIntoView({ block: 'center', behavior: 'smooth' });
    // The card's short tint, as a search hit's: set again so it plays again.
    el.removeAttribute('data-flash');
    requestAnimationFrame(() => el.setAttribute('data-flash', ''));
    setTimeout(() => el.removeAttribute('data-flash'), 1700);
  };
  return (
    <Popover.Root open={open} onOpenChange={setOpen}>
      <Popover.Trigger asChild>
        <IconButton aria-label="Needs you" className="head-button needs-button">
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
            <circle cx="18.5" cy="5.5" r="2.5" />
            <path d="M7.5 13h7M7.5 17h4" />
          </svg>
          {rows.length > 0 && <span className="head-dot" />}
        </IconButton>
      </Popover.Trigger>
      <Popover.Portal>
        <Popover.Content
          className="popover-panel needs"
          side="bottom"
          align="end"
          sideOffset={8}
          collisionPadding={16}
          role="dialog"
          aria-label="Needs you"
        >
          <h3>Needs you {rows.length > 0 && <span className="needs-count">{rows.length}</span>}</h3>
          {rows.length === 0 && <p className="needs-none">Nothing</p>}
          {rows.map((r) => (
            <button key={r.key} type="button" className="needs-row" onClick={() => go(r.card)}>
              <span className="needs-face">
                <Face agent={people.face(r.agent)} size={20} />
              </span>
              <span className="needs-main">
                <span className="needs-top">
                  <b>{people.name(r.agent)}</b>
                  {r.what && <span className="needs-what">{r.what}</span>}
                  {r.time && <span className="room-time">{r.time}</span>}
                </span>
                <span className="needs-text" data-code={r.what ? '' : undefined}>
                  {r.text}
                </span>
              </span>
            </button>
          ))}
        </Popover.Content>
      </Popover.Portal>
    </Popover.Root>
  );
}

/** A question's first line, as plain words: its markdown marks taken out. */
const firstLine = (text: string) =>
  (text.split('\n').find((l) => l.trim()) ?? '').replace(/[*_`#>]/g, '').trim();
