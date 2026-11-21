import { File as FileIcon } from 'lucide-react';
import { type ReactNode, useEffect, useId, useRef, useState } from 'react';
import { api, type Failure } from '../api';
import { useCard } from '../components/Corner';
import type { Opened } from './Files';

// The service is asked once typing stops for this long.
const WAIT = 150;

interface Props {
  chat: string;
  /** The box's icon and its words when empty. */
  icon: ReactNode;
  label: string;
  onOpen: (file: Opened) => void;
  /** What the box sits above, put aside while it holds words and kept as it was. */
  children: ReactNode;
}

/** A box that finds the chat's files by name: Home's "Find a file" and the tree's "Filter files". */
export function Find({ chat, icon, label, onOpen, children }: Props) {
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
  const hits = found ?? [];
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
              open(hits[cursor]);
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
          {hits.map((file, i) => (
            <button
              type="button"
              key={file.path}
              id={`${id}-${i}`}
              role="option"
              aria-selected={i === cursor}
              tabIndex={-1}
              onClick={() => open(file)}
            >
              <FileIcon aria-hidden="true" />
              <span>{file.crumbs.at(-1)}</span>
              <small>{file.crumbs.slice(0, -1).join(' / ')}</small>
            </button>
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
