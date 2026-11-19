import { useState } from 'react';
import type { PickedName, ReplyTarget, Task } from '../api';
import type { Person } from '../components/Composer';
import { kept as keptPicks, type Picked } from '../components/mention';
import type { Draft, Refusal } from '../tasks/model';
import type { Answer } from './messages';
import { addressed } from './recipient';
import type { ChatState } from './state';
import type { Box } from './ThreadBox';

type Drafts = {
  get?: (topic: string) => { text: string; picks: Picked[] } | undefined;
  set?: (topic: string, text: string, picks: Picked[]) => void;
};

interface Room {
  state: ChatState;
  /** Who the box's "@" can name, as the main box's. */
  people: Person[];
  archived: boolean;
  drafts: Drafts;
  send: (
    text: string,
    agent?: string,
    picks?: PickedName[],
    retrying?: undefined,
    to?: ReplyTarget,
  ) => unknown;
  /** Talks a waiting question over, as the Tasks view's Discuss does (0065). */
  discuss?: (task: Task, draft: Draft) => Promise<Refusal | null>;
  /** Opens the thread the box goes in. */
  show: (topic: string) => void;
}

const NONE: Picked[] = [];

/** The box open for a reply: what it answers, and the Reply that opened it, which gets focus back. */
export interface Opened {
  answer: Answer;
  opener: HTMLElement | null;
}

/**
 * Replying in a thread (0076): the one box, open for one thread at a time, with that thread's own draft. Its Send puts
 * a reply at the thread's end, answering what Reply was pressed on; in a waiting question's thread it talks the
 * question over instead, and the question stays open. Closing it gives focus back to the Reply that opened it.
 */
export function useReplying(
  [open, setOpen]: [Opened | undefined, (open: Opened | undefined) => void],
  { state, people, archived, drafts, send, discuss, show }: Room,
) {
  const [refused, setRefused] = useState<{ topic: string; why: string; ref: string | null }>();
  // Drafts the app doesn't keep (a chat shown alone) live while the chat is open.
  const [own, setOwn] = useState(new Map<string, { text: string; picks: Picked[] }>());
  const draftOf = (topic: string) => (drafts.get ? drafts.get(topic) : own.get(topic));
  const setDraft = (topic: string, text: string, picks: Picked[]) =>
    drafts.set ? drafts.set(topic, text, picks) : setOwn((m) => new Map(m).set(topic, { text, picks }));

  const close = () => {
    const opener = open?.opener;
    setOpen(undefined);
    requestAnimationFrame(() => {
      if (opener?.isConnected) opener.focus();
    });
  };
  const waiting = (topic: string) => {
    const n = topic.startsWith('t:') ? Number(topic.slice(2)) : Number.NaN;
    return state.tasks.find((t) => t.number === n && t.kind === 'person' && t.state === 'ready' && t.giver);
  };

  let box: Box | undefined;
  if (open) {
    const { answer } = open;
    const { text, picks } = draftOf(answer.topic) ?? { text: '', picks: NONE };
    const fallback = people.find((p) => p.id === answer.agent);
    // The agent it answers first, then each agent picked in it (0028).
    const to = addressed(picks, people, fallback, { fresh: false, reply: true });
    const onSend = async () => {
      if (!text.trim()) return;
      setDraft(answer.topic, '', []);
      setRefused(undefined);
      const question = waiting(answer.topic);
      close();
      if (question && discuss) {
        const others = to.slice(1).map((p) => p.id);
        const talk = {
          values: [],
          notes: '',
          discuss: true,
          message: text,
          ...(others.length ? { to: others } : {}),
        };
        const refusal = await discuss(question, talk);
        if (!refusal) return;
        // Not taken: the words come back to the box, with why and the reference.
        setDraft(answer.topic, text, picks);
        setRefused({ topic: answer.topic, why: refusal.why, ref: refusal.ref });
        setOpen({ answer, opener: null });
        return;
      }
      const target: ReplyTarget =
        answer.task != null ? { reply_task: answer.task } : { reply_to: answer.message };
      send(text, to[0]?.id, keptPicks(picks), undefined, target);
    };
    box = {
      topic: answer.topic,
      draft: text,
      picks,
      onDraft: (t, p) => setDraft(answer.topic, t, p),
      onSend,
      onClose: close,
      people,
      to,
      hint: 'Reply',
      refused: refused?.topic === answer.topic ? refused : undefined,
    };
  }
  return {
    box,
    /** Opens the box for a reply answering `answer`, in its thread, opened if folded. */
    open: (answer: Answer, opener: HTMLElement) => {
      show(answer.topic);
      setRefused(undefined);
      setOpen({ answer, opener });
    },
    /** A reply reaches only an agent still in the chat's reach, and nothing is sent in an archived chat. */
    can: (answer: Answer) =>
      !archived && !!answer.agent && people.some((p) => p.id === answer.agent && !p.out),
  };
}
