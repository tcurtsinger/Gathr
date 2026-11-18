import type { ReactNode } from 'react';
import type { PickedName } from '../api';
import './picked.css';

/**
 * The person's sent words, each name they picked from the "@" list blue, as it was in the message box. Only the
 * saved spans are coloured, whatever the agent is called now: a name only typed stays plain (0028, 0072).
 */
export function PickedText({ text, picks = [] }: { text: string; picks?: readonly PickedName[] }) {
  const pieces: ReactNode[] = [];
  let from = 0;
  for (const p of [...picks].sort((a, b) => a.at - b.at)) {
    if (p.at < from || p.at + p.len > text.length) continue;
    pieces.push(
      text.slice(from, p.at),
      <span key={p.at} className="picked-name">
        {text.slice(p.at, p.at + p.len)}
      </span>,
    );
    from = p.at + p.len;
  }
  pieces.push(text.slice(from));
  return <p className="picked-text">{pieces}</p>;
}
