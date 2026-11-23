import { Folder, MoreHorizontal, Pencil, Plug, Trash2, User } from 'lucide-react';
import { createContext, type ReactNode, useCallback, useEffect, useRef, useState } from 'react';
import { Button } from '@/components/ui/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { api, type Failure } from '../api';
import { Confirm } from '../components/Confirm';
import { useCard } from '../components/Corner';
import { Face } from '../components/Face';
import { IconButton } from '../components/IconButton';
import { ago } from '../components/marks';
import { listen } from '../live';
import { Screen } from '../Screen';
import type { MemoryNote, MemoryPlace, MemoryShown } from './places';
import './memory.css';

/** Opens the Memory screen at a place (`you`, `project:<id>`, `agent:<id>`, `tool:<id>`): a noted line in a chat calls it. */
export const OpenMemory = createContext<((place: string) => void) | undefined>(undefined);

interface Props {
  /** The place open. */
  at: string;
  onAt: (place: string) => void;
}

/**
 * The Memory screen, in place of the chat list and the chat: About you, each project, agent and tool in its sidebar; a
 * place's notes in the main panel, each with Edit and Remove, and a box to add one.
 */
export function Memory({ at, onAt }: Props) {
  const [places, setPlaces] = useState<MemoryPlace[]>();
  const [shown, setShown] = useState<MemoryShown>();
  const [failure, setFailure] = useState<Failure | null>(null);
  useCard(failure);
  const [removing, setRemoving] = useState<{ note: MemoryNote; opener: HTMLElement | null } | null>(null);

  const loadPlaces = useCallback(() => api.memory().then((m) => setPlaces(m.places), setFailure), []);
  useEffect(() => {
    loadPlaces();
    return listen((e) => {
      if (['memory', 'agents', 'project', 'tools', 'lagged'].includes(e.type)) loadPlaces();
    });
  }, [loadPlaces]);
  useEffect(() => {
    let open = true;
    const load = () =>
      api.memoryPlace(at).then(
        (s) => open && setShown(s),
        (f: Failure) => open && setFailure(f),
      );
    load();
    const stop = listen((e) => {
      // A tool renamed keeps its notes: its page takes its new name.
      if (e.type === 'memory' || e.type === 'tools' || e.type === 'lagged') load();
    });
    return () => {
      open = false;
      stop();
    };
  }, [at]);
  // A project, agent or tool removed while its page is open: back to About you.
  useEffect(() => {
    if (places && !places.some((p) => p.id === at)) onAt('you');
  }, [places, at, onAt]);

  const page = shown?.id === at ? shown : undefined;
  const kind = places?.find((p) => p.id === at)?.kind;
  const row = (place: MemoryPlace, mark: ReactNode) => (
    <div key={place.id} className="chatrow" data-current={place.id === at || undefined}>
      <button
        type="button"
        className="chat-link"
        aria-current={place.id === at ? 'page' : undefined}
        onClick={() => onAt(place.id)}
      >
        {mark}
        <span>{place.name}</span>
      </button>
    </div>
  );
  const of = (k: MemoryPlace['kind']) => places?.filter((p) => p.kind === k) ?? [];
  const projects = of('project');
  const agents = of('agent');
  const tools = of('tool');

  return (
    <>
      <Screen
        name="Memory"
        nav={
          <div className="chatlist memory-nav">
            <section>
              {of('you').map((p) => row(p, <User aria-hidden="true" className="memory-nav-icon" />))}
            </section>
            {projects.length > 0 && (
              <section>
                <h2>Projects</h2>
                {projects.map((p) => row(p, <Folder aria-hidden="true" className="memory-nav-icon" />))}
              </section>
            )}
            {agents.length > 0 && (
              <section>
                <h2>Agents</h2>
                {agents.map((p) =>
                  row(p, <Face agent={{ id: p.id.slice('agent:'.length), name: p.name }} size={20} />),
                )}
              </section>
            )}
            {tools.length > 0 && (
              <section>
                <h2>Tools</h2>
                {tools.map((p) => row(p, <Plug aria-hidden="true" className="memory-nav-icon" />))}
              </section>
            )}
          </div>
        }
      >
        {page && (
          <>
            <h1>{page.name}</h1>
            <p className="muted memory-lede">{lede(kind, page.name)}</p>
            {page.notes.length === 0 ? (
              <p className="muted memory-empty">Nothing yet</p>
            ) : (
              <ul className="memory-notes">
                {page.notes.map((n) => (
                  <NoteRow
                    key={n.text}
                    note={n}
                    onSave={(text) => api.changeNote(at, n.text, text).then(setShown)}
                    onRemove={(opener) => setRemoving({ note: n, opener })}
                  />
                ))}
              </ul>
            )}
            <AddNote key={at} onAdd={(text) => api.addNote(at, text).then(setShown)} />
          </>
        )}
      </Screen>
      {removing && (
        <Confirm
          question="Remove note?"
          body={removing.note.text}
          verb="Remove"
          danger
          opener={removing.opener}
          onCancel={() => setRemoving(null)}
          onConfirm={() =>
            api.removeNote(at, removing.note.text).then((s) => {
              setShown(s);
              setRemoving(null);
            })
          }
        />
      )}
    </>
  );
}

/** The sentence under a place's title. */
function lede(kind: MemoryPlace['kind'] | undefined, name: string) {
  if (kind === 'project') return 'What agents know about this project.';
  if (kind === 'agent') return `What ${name} has learned. It goes with ${name} into every project.`;
  if (kind === 'tool')
    return 'What agents have learned about using this tool. Every agent that can use it gets these.';
  return 'What every agent knows about you. Agents add to it as they learn.';
}

/** Who added it and how long ago: "Ann · 3d", "You", or nothing for a line written in the file. */
function byline(note: MemoryNote) {
  const when = note.when ? Date.parse(note.when) : Number.NaN;
  if (!note.by || note.by === 'You' || Number.isNaN(when)) return note.by;
  return `${note.by} · ${ago(when, Date.now())}`;
}

interface RowProps {
  note: MemoryNote;
  /** Settles once it's saved; a refusal is shown under the box, which keeps what was typed. */
  onSave: (text: string) => Promise<unknown>;
  onRemove: (opener: HTMLElement | null) => void;
}

/** A note: its text and who added it, with Edit and Remove in its More; Edit turns it into a box. */
function NoteRow({ note, onSave, onRemove }: RowProps) {
  const [editing, setEditing] = useState(false);
  const trigger = useRef<HTMLButtonElement>(null);
  const by = byline(note);
  if (editing)
    return (
      <li className="memory-note">
        <NoteForm
          start={note.text}
          verb="Save"
          onCancel={() => setEditing(false)}
          onDone={(text) => onSave(text).then(() => setEditing(false))}
        />
      </li>
    );
  return (
    <li className="memory-note">
      <div className="memory-words">
        <span>{note.text}</span>
        {by && <small className="muted">{by}</small>}
      </div>
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <IconButton ref={trigger} aria-label="More" className="memory-more">
            <MoreHorizontal />
          </IconButton>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" sideOffset={8}>
          <DropdownMenuItem onSelect={() => setEditing(true)}>
            <Pencil />
            Edit
          </DropdownMenuItem>
          <DropdownMenuSeparator />
          <DropdownMenuItem variant="destructive" onSelect={() => onRemove(trigger.current)}>
            <Trash2 />
            Remove
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    </li>
  );
}

/** Under the notes: a box and Add; Enter adds too. */
function AddNote({ onAdd }: { onAdd: (text: string) => Promise<unknown> }) {
  return <NoteForm start="" verb="Add" placeholder="Add a note" onDone={onAdd} clears />;
}

interface FormProps {
  start: string;
  verb: 'Save' | 'Add';
  placeholder?: string;
  onCancel?: () => void;
  onDone: (text: string) => Promise<unknown>;
  /** Empties the box once done. */
  clears?: boolean;
}

/** A note's one box with its buttons, and why the service refused it under them. */
function NoteForm({ start, verb, placeholder, onCancel, onDone, clears }: FormProps) {
  const [text, setText] = useState(start);
  const [busy, setBusy] = useState(false);
  const [why, setWhy] = useState<string | null>(null);
  const done = () => {
    setBusy(true);
    setWhy(null);
    onDone(text).then(
      () => {
        setBusy(false);
        if (clears) setText('');
      },
      (f: Failure) => {
        setBusy(false);
        setWhy(f.why ?? f.error);
      },
    );
  };
  return (
    <form
      className="memory-form"
      onSubmit={(e) => {
        e.preventDefault();
        done();
      }}
    >
      <div className="memory-box">
        <input
          value={text}
          placeholder={placeholder}
          aria-label={verb === 'Add' ? 'Add a note' : 'Note'}
          aria-invalid={why ? true : undefined}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={(e) => e.key === 'Escape' && onCancel?.()}
          // biome-ignore lint/a11y/noAutofocus: Edit opens the box for typing.
          autoFocus={!!onCancel}
        />
        {onCancel && (
          <Button type="button" size="sm" variant="outline" onClick={onCancel}>
            Cancel
          </Button>
        )}
        <Button
          type="submit"
          size="sm"
          variant={onCancel ? 'default' : 'outline'}
          disabled={busy || !text.trim()}
        >
          {verb}
        </Button>
      </div>
      {why && <p className="memory-why">{why}</p>}
    </form>
  );
}
