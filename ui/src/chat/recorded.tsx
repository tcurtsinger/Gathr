// For tests: the service's real answers, recorded by its own tests (`GATHR_RECORD_UI`, 0075), drawn as the chat draws
// them: the threads, their folds, and the message box under them.
import { type ReactNode, useMemo, useRef } from 'react';
import type { History, Named } from '../api';
import { useFolds } from './folds';
import { toThread } from './messages';
import askAtWork from './recorded/ask-at-work.json';
import askDone from './recorded/ask-done.json';
import oldChat from './recorded/old-chat.json';
import questionDiscussed from './recorded/question-discussed.json';
import reviewAndFix from './recorded/review-and-fix.json';
import { type ChatState, fromHistory } from './state';
import { type Actions, Thread } from './Thread';

const recorded = {
  /** An ask whose plan was approved: the lead's two parts, Ann at work on the first, the second not started. */
  askAtWork,
  /** The same ask once all is in: a part done, a part cancelled, the lead's hear-backs, the ask's result. */
  askDone,
  /** A chat from before asks and `opened_by`: its turns tied by `task` and `answers` alone. */
  oldChat,
  /** A member's question the person discussed: it waits on them again. */
  questionDiscussed,
  /** A task with a PR, its review left with a note, and the fix Gathr gave after it. */
  reviewAndFix,
};

type Work = NonNullable<Parameters<typeof fromHistory>[1]>;

/** A recorded chat as the window holds it once loaded. */
export function chat(name: keyof typeof recorded): ChatState {
  const { history, work } = recorded[name] as unknown as { history: History; work: Work };
  return fromHistory(history, work);
}

/** The chat's agents, by their names as the chat's members carry them. */
export const named = (state: ChatState): Named[] =>
  state.members.map((m) => ({
    id: m.agent,
    name: m.name,
    face_seed: m.name,
    gender: null,
    skin: null,
    background: null,
  }));

export const noActions = new Proxy({}, { get: () => () => {} }) as Actions;

/** A chat drawn as ChatView draws it: the folds kept while it is open, a hit opened where it is. */
export function View({
  state,
  actions = noActions,
  found,
  width = 900,
  children,
}: {
  state: ChatState;
  actions?: Partial<Actions>;
  found?: string;
  width?: number;
  children?: ReactNode;
}) {
  const cache = useRef(new Map());
  const threads = useFolds();
  const { folds, start } = threads;
  const agents = useMemo(() => named(state), [state]);
  const timeline = useMemo(
    () => toThread(state, agents, [], cache.current, { ...folds, start: start.current }),
    [state, agents, folds, start],
  );
  threads.settle(timeline, true);
  // biome-ignore lint/correctness/useExhaustiveDependencies: placed once, as the chat places a hit when it loads
  const hit = useMemo(() => found && { ...timeline.find(found), at: 1 }, [found]);
  threads.open(hit || undefined);
  return (
    <div style={{ height: 900, width, display: 'flex' }}>
      <Thread
        messages={timeline.items}
        running={false}
        actions={{ ...noActions, fold: threads.fold, earlier: threads.earlier, ...actions }}
        empty={false}
        target={hit || undefined}
      >
        {children}
      </Thread>
    </div>
  );
}
