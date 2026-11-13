import { Popover } from 'radix-ui';
import type { WaitingLine as Line, Waiting } from '../api';
import { Elapsed } from './Working';
import './summary.css';
import './waiting.css';

type Moves = Line['moves'];

/** The groups of the list, in the order the service sorts the lines. */
const GROUPS: [Moves, string][] = [
  ['you', 'Waiting on you'],
  ['nobody', 'Not moving'],
  ['itself', 'Moving by itself'],
  ['others', 'Waiting on the above'],
];

const count = (n: number, one: string) => `${n} ${one}${n === 1 ? '' : 's'}`;

/** What waits on the person, counted by kind: "2 questions, 1 approval". */
function asks(lines: Line[]): string {
  const of = (kind: string) => lines.filter((l) => l.kind === kind).length;
  const [questions, approvals, signIns] = [of('question'), of('approval'), of('sign_in')];
  const paused = lines.length - questions - approvals - signIns;
  return [
    questions && count(questions, 'question'),
    approvals && count(approvals, 'approval'),
    signIns && count(signIns, 'sign-in'),
    paused && `${paused} paused`,
  ]
    .filter(Boolean)
    .join(', ');
}

/**
 * The line's words, the first that applies: what waits on the person; else the thing that has waited
 * longest among those that move by themselves; else what nobody is on. `more` is every other line.
 */
export function headline(lines: Line[]) {
  const you = lines.filter((l) => l.moves === 'you');
  if (you.length)
    return {
      moves: 'you' as Moves,
      lead: 'Waiting on you',
      words: asks(you),
      more: lines.length - you.length,
    };
  const moving = lines.find((l) => l.moves === 'itself');
  const first = moving ?? lines.find((l) => l.moves === 'nobody') ?? lines[0];
  return {
    moves: (moving ? 'itself' : 'nobody') as Moves,
    lead: moving ? 'Waiting' : 'Not moving',
    words: first.what,
    since: first.since_us ?? undefined,
    more: lines.length - 1,
  };
}

/**
 * What the chat waits on, at the foot of the chat while no turn runs (0050; the owner's pick, where
 * "Working" shows): one line, the most pressing first, that opens the whole list. Nothing when the
 * answer has no line.
 */
export function WaitingLine({ waiting }: { waiting?: Waiting }) {
  const lines = waiting?.lines ?? [];
  if (lines.length === 0) return null;
  const said = headline(lines);
  const words = `${said.lead}: ${said.words}`;
  return (
    <Popover.Root>
      <div className="working waiting-line">
        <Popover.Trigger asChild>
          <button type="button" aria-haspopup="dialog" title={words}>
            <span className="waiting-mark" data-moves={said.moves} aria-hidden="true" />
            <span className="waiting-words">{words}</span>
            {said.since !== undefined && (
              // Not read out: it is redrawn as time passes.
              <span className="working-time" aria-hidden="true">
                <Elapsed since={said.since / 1000} to="minute" />
              </span>
            )}
            {said.more > 0 && <span className="waiting-more">and {said.more} more</span>}
          </button>
        </Popover.Trigger>
        {/* Beside the button, not around it: a live region would read the control out each time it changed. */}
        <span className="visually-hidden" role="status">
          {words}
        </span>
      </div>
      <Popover.Portal>
        <Popover.Content
          className="summary waiting-list"
          side="top"
          align="start"
          sideOffset={8}
          collisionPadding={16}
          role="dialog"
          aria-label="What this chat waits on"
        >
          {GROUPS.map(([moves, heading]) => {
            const of = lines.filter((l) => l.moves === moves);
            return (
              of.length > 0 && (
                <section key={moves} aria-label={heading}>
                  <h3>
                    {heading}
                    <small className="summary-note">{of.length}</small>
                  </h3>
                  <ul>
                    {of.map((l) => (
                      <Row key={`${l.kind}-${l.agent}-${l.task}-${l.since_us}-${l.what}`} line={l} />
                    ))}
                  </ul>
                </section>
              )
            );
          })}
        </Popover.Content>
      </Popover.Portal>
    </Popover.Root>
  );
}

/** One line of the list: what it is, for how long, and what its last look saw or why its looks fail. */
function Row({ line }: { line: Line }) {
  return (
    <li>
      <span className="waiting-what">{line.what}</span>
      <span className="waiting-meta">
        {line.since_us !== null && (
          <span>
            <Elapsed since={line.since_us / 1000} to="minute" bare />
          </span>
        )}
        {line.seen && (
          <span>
            {line.looked_us !== null && (
              <>
                Last look <Elapsed since={line.looked_us / 1000} to="minute" bare /> ago:{' '}
              </>
            )}
            {line.seen}
          </span>
        )}
        {line.error && <span className="waiting-error">Its looks fail: {line.error}</span>}
      </span>
    </li>
  );
}
