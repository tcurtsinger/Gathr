import { useEffect, useRef, useState, useSyncExternalStore } from 'react';
import { api } from '../api';
import { listen as live } from '../live';

/** A step an agent took with the driver (`tool`), on an element (`target`) or typing `text`. */
export interface ComputerStep {
  at: string;
  tool: string;
  target: string | null;
  text: string | null;
  /** Only "paused": made while Pause was on. */
  refused: string | null;
  failed: string | null;
}

/** An agent that used the computer: the window it last worked in, its latest picture and its last steps, oldest first. */
export interface ComputerUser {
  agent: string;
  chat: string;
  window: string | null;
  /** Changes with each new picture; null before the first. */
  shot: number | null;
  /** Where its last pointer step landed, in the picture's pixels. */
  mark: { x: number; y: number } | null;
  steps: ComputerStep[];
}

/** Computer use, the same in every chat (Shared computer-use/api.md): its tool's switch, its driver, and Pause. */
export interface Computer {
  tool: string;
  on: boolean;
  driver: 'missing' | 'fetching' | 'ready' | 'failed';
  why: string | null;
  paused: boolean;
  /** The one that acted last first. */
  agents: ComputerUser[];
}

let state: Computer | null = null;
const heard = new Set<() => void>();
let stop: (() => void) | null = null;

// Each read or Pause is numbered when it is sent: an answer to one sent before the last shown is stale (a read sent
// before Pause can come back after it, still saying unpaused).
let sent = 0;
let last = 0;
function ask(call: Promise<Computer>) {
  const n = ++sent;
  return call.then((next) => {
    if (n < last) return;
    last = n;
    state = next;
    for (const heed of heard) heed();
  });
}

const load = () => ask(api.computer()).catch(() => {});

function subscribe(heed: () => void) {
  heard.add(heed);
  if (!stop) {
    load();
    stop = live((e) => {
      if (e.type === 'computer' || e.type === 'lagged') load();
    });
  }
  return () => {
    heard.delete(heed);
    if (heard.size) return;
    stop?.();
    stop = null;
  };
}

/** Computer use as the service has it, kept up to date while shown; null until it has answered. */
export const useComputer = () => useSyncExternalStore(subscribe, () => state);

/** Pauses or resumes every agent's computer use. */
export const pauseComputer = (paused: boolean) => ask(api.pauseComputer(paused));

/** An agent's latest picture, as a local address; the one before stays shown until the new one has come. */
export function useShot(agent: string, shot: number | null) {
  const [url, setUrl] = useState<string>();
  const shown = useRef<string>(undefined);
  useEffect(() => {
    if (shot === null) return;
    let gone = false;
    fetch(`/api/computer/agents/${encodeURIComponent(agent)}/shot?n=${shot}`)
      .then((r) => (r.ok ? r.blob() : Promise.reject()))
      .then(
        (blob) => {
          if (gone) return;
          if (shown.current) URL.revokeObjectURL(shown.current);
          shown.current = URL.createObjectURL(blob);
          setUrl(shown.current);
        },
        () => {},
      );
    return () => {
      gone = true;
    };
  }, [agent, shot]);
  useEffect(
    () => () => {
      if (shown.current) URL.revokeObjectURL(shown.current);
    },
    [],
  );
  return url;
}
