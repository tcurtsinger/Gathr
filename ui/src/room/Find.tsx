import { File as FileIcon } from 'lucide-react';
import { type ReactNode, useEffect, useId, useRef, useState } from 'react';
import { api, type Changes, type Failure } from '../api';
import { useCard } from '../components/Corner';
import { Face } from '../components/Face';
import type { Opened } from './Files';

// The service is asked once typing stops for this long.
const WAIT = 150;

interface Props {
  chat: string;
  /** The box's icon and its words when empty. */
  icon: ReactNode;
  label: string;
  onOpen: (file: Opened) => void;
  /** The project folder's name and what each agent changed, to show each file once. */
  project?: string;
  changes?: Changes;
  /** What the box sits above, put aside while it holds words and kept as it was. */
  children: ReactNode;
}

/** A box that finds the chat's files by name: Home's "Find file" and the tree's "Filter files". */
export function Find({ chat, icon, label, onOpen, project, changes, children }: Props) {
  const [q, setQ] = useState('');
  const [found, setFound] = useState<Opened[]>();
  const [cursor, setCursor] = useState(0);
  const [failure, setFailure] = useState<Failure>();
  useCard(failure);
  const asked = useRef(0);
  const id = useId();
  const words = q.trim();

  useEffect(() => {
    const n = ++asked.current;
    // The last answer stays until the next one comes.
    if (!words) {
      setFound(undefined);
      return;
    }
    const wait = setTimeout(() => {
      api.find(chat, words).then(
        (files) => {
          if (n !== asked.current) return;
          setFound(files);
          setCursor(0);
        },
        (f: Failure) => n === asked.current && setFailure(f),
      );
    }, WAIT);
    return () => clearTimeout(wait);
  }, [chat, words]);

  const open = (file: Opened) => {
    setQ('');
    onOpen(file);
  };
  const hits = collapse(found ?? [], project, changes);
  return (
    <>
      <label className="search-box find-box">
        {icon}
        <input
          value={q}
          onChange={(e) => setQ(e.target.value)}
          placeholder={label}
          aria-label={label}
          autoComplete="off"
          role="combobox"
          aria-autocomplete="list"
          aria-expanded={hits.length > 0}
          aria-controls={`${id}-found`}
          aria-activedescendant={hits.length ? `${id}-${cursor}` : undefined}
          onKeyDown={(e) => {
            if (e.key === 'Enter' && hits[cursor]) {
              e.preventDefault();
              open(hits[cursor].file);
            } else if ((e.key === 'ArrowDown' || e.key === 'ArrowUp') && hits.length) {
              e.preventDefault();
              const next = (cursor + (e.key === 'ArrowDown' ? 1 : hits.length - 1)) % hits.length;
              setCursor(next);
              document.getElementById(`${id}-${next}`)?.scrollIntoView({ block: 'nearest' });
            }
          }}
        />
      </label>
      {words && (
        <div className="find-list" id={`${id}-found`} role="listbox" aria-label={label}>
          {hits.map(({ file, copies }, i) => (
            // biome-ignore lint/a11y/useKeyWithClickEvents: the box's Enter opens the row under the cursor
            <div
              key={file.path}
              id={`${id}-${i}`}
              role="option"
              aria-selected={i === cursor}
              tabIndex={-1}
              onClick={() => open(file)}
            >
              <FileIcon aria-hidden="true" />
              <span>{marked(file.crumbs.at(-1) ?? '', words)}</span>
              <small>{where(file, project, changes)}</small>
              {copies.map((c) => (
                <button
                  type="button"
                  key={c.agent.id}
                  aria-label={`${c.agent.name}'s copy`}
                  onClick={(e) => {
                    e.stopPropagation();
                    open(c.file);
                  }}
                >
                  <Face agent={c.agent} size={16} />
                </button>
              ))}
            </div>
          ))}
          {found?.length === 0 && <p role="status">No files found</p>}
        </div>
      )}
      <div className="find-rest" hidden={!!words}>
        {children}
      </div>
    </>
  );
}

interface Row {
  file: Opened;
  /** The agents' copies that differ: each agent that changed the file in its own folder. */
  copies: { agent: { id: string; name: string }; file: Opened }[];
}

/**
 * Each file found once: an agent's copy goes under the project's file of the same path, with its face when the agent
 * changed it, and is left out when it didn't. A file only agents have opens the first agent's copy.
 */
function collapse(found: Opened[], project?: string, changes?: Changes): Row[] {
  const agents = new Map((changes?.agents ?? []).map((a) => [a.name, a]));
  const rel = (f: Opened) => f.crumbs.slice(1).join('/');
  const own = new Set(found.filter((f) => f.crumbs[0] === project).map(rel));
  const rows: Row[] = [];
  const at = new Map<string, Row>();
  for (const file of found) {
    const agent = agents.get(file.crumbs[0]);
    const path = rel(file);
    if (!agent) {
      rows.push({ file, copies: [] });
      if (file.crumbs[0] === project) at.set(path, rows[rows.length - 1]);
      continue;
    }
    const changed = agent.files.some((c) => c.path.replaceAll('\\', '/') === path);
    const copy = { agent: { id: agent.id, name: agent.name }, file };
    const row = at.get(path);
    if (row) {
      if (changed) row.copies.push(copy);
    } else if (changed || !own.has(path)) {
      const made = { file, copies: changed ? [copy] : [] };
      rows.push(made);
      at.set(path, made);
    }
  }
  return rows;
}

/** Where a file is: its folders inside the project or an agent's folder, else from its top folder. */
function where(file: Opened, project?: string, changes?: Changes) {
  const top = file.crumbs[0];
  const inside = top === project || changes?.agents.some((a) => a.name === top);
  return file.crumbs.slice(inside ? 1 : 0, -1).join(' / ');
}

/** The name with the words found in it in bold. */
function marked(name: string, words: string) {
  const at = name.toLowerCase().indexOf(words.toLowerCase());
  if (at < 0) return name;
  return (
    <>
      {name.slice(0, at)}
      <b>{name.slice(at, at + words.length)}</b>
      {name.slice(at + words.length)}
    </>
  );
}
