import './initial.css';

/** As the chat's avatars show them (speaker-identity): the first letters of the first two words. */
export const initials = (name: string) =>
  name
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((w) => [...w][0]?.toUpperCase())
    .join('');

interface Props {
  name: string;
  /** 18px in a stack or a tab, 24px in a row, 36px heading the agent's panel. */
  size?: 's' | 'm' | 'l';
  /** A turn of its is running: an attention ring turns around it. */
  busy?: boolean;
}

/** An agent's initials in a circle. Whatever names the agent sits beside it, so it isn't read out. */
export function Initial({ name, size = 'm', busy }: Props) {
  return (
    <span className="initial" data-size={size} data-busy={busy || undefined} aria-hidden="true">
      {initials(name)}
    </span>
  );
}

/** Initials overlapped into one compact element, in the order given. */
export function InitialStack({
  agents,
  needs,
}: {
  agents: { agent: string; name: string; busy?: boolean }[];
  /** Someone needs the person: one attention dot on the stack's corner. */
  needs?: boolean;
}) {
  return (
    <span className="initial-stack" data-needs={needs || undefined} aria-hidden="true">
      {agents.map((a) => (
        <Initial key={a.agent} name={a.name} size="s" busy={a.busy} />
      ))}
      {needs && <i className="attention" />}
    </span>
  );
}
