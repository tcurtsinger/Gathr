import { ChevronDown } from 'lucide-react';
import { useContext, useState } from 'react';
import { Button } from '@/components/ui/button';
import type { ToolBody } from '../api';
import { ChangesMark } from '../components/ChangesMark';
import { OpenFile, pathKey } from '../components/files';
import type { Step } from './shape';
import './files-card.css';

type Changed = NonNullable<ToolBody['changed']>[number];

/** The rows shown before "Show more". */
const FIRST = 3;

/** The files a turn's steps changed: one row each, in the order first changed, with the sums of its edits. */
function changedOf(steps: Step[]): Changed[] {
  const files = new Map<string, Changed>();
  for (const s of steps) {
    if ('note' in s) continue;
    for (const c of s.changed ?? []) {
      const key = pathKey(c.full ?? c.path);
      const had = files.get(key);
      files.set(
        key,
        had ? { ...had, added: had.added + c.added, removed: had.removed + c.removed } : { ...c },
      );
    }
  }
  return [...files.values()];
}

/**
 * Lines added and removed, side by side; a side with none says nothing, unless both have none. A file's row keeps both
 * places, so the numbers stay in their columns.
 */
function Counts({ added, removed, each }: { added: number; removed: number; each?: boolean }) {
  const plus = added > 0 || removed === 0;
  return (
    <span className={each ? 'files-card-counts files-card-each' : 'files-card-counts'}>
      {(plus || each) && <span className="files-card-plus">{plus && `+${added.toLocaleString('en')}`}</span>}
      {(removed > 0 || each) && (
        <span className="files-card-minus">{removed > 0 && `−${removed.toLocaleString('en')}`}</span>
      )}
    </span>
  );
}

interface Props {
  steps: Step[];
  /** Whose turn: a path in its folder opens there. */
  agent: string;
  /** Opens the chat's Changes tab. */
  onView?: () => void;
}

/**
 * The files a turn changed, as one card under its words: how many and the lines added and removed, View changes, then
 * a row per file. The first three show; the rest open in place.
 */
export function FilesCard({ steps, agent, onView }: Props) {
  const open = useContext(OpenFile);
  const [all, setAll] = useState(false);
  const files = changedOf(steps);
  if (!files.length) return null;
  const added = files.reduce((n, f) => n + f.added, 0);
  const removed = files.reduce((n, f) => n + f.removed, 0);
  const more = files.length - FIRST;
  const shown = all ? files : files.slice(0, FIRST);
  return (
    <section className="files-card" aria-label="Changed files">
      <div className="files-card-head">
        <span className="files-card-mark">
          <ChangesMark />
        </span>
        <span className="files-card-sum">
          <b>
            Edited {files.length} {files.length === 1 ? 'file' : 'files'}
          </b>
          {files.length > 1 && <Counts added={added} removed={removed} />}
        </span>
        {onView && (
          <Button type="button" size="xs" variant="outline" onClick={onView}>
            View changes
          </Button>
        )}
      </div>
      <ul className="files-card-list">
        {shown.map((f) => {
          const row = (
            <>
              <Path path={f.path} />
              <Counts added={f.added} removed={f.removed} each />
            </>
          );
          return (
            <li key={pathKey(f.full ?? f.path)}>
              {open ? (
                <button
                  type="button"
                  className="files-card-row"
                  onClick={() => open(f.full ?? f.path, agent)}
                >
                  {row}
                </button>
              ) : (
                <span className="files-card-row">{row}</span>
              )}
            </li>
          );
        })}
        {more > 0 && (
          <li>
            <button
              type="button"
              className="files-card-row files-card-more"
              aria-expanded={all}
              onClick={() => setAll(!all)}
            >
              {all ? 'Show fewer' : `Show ${more} more ${more === 1 ? 'file' : 'files'}`}
              <ChevronDown aria-hidden="true" />
            </button>
          </li>
        )}
      </ul>
    </section>
  );
}

/** A file's folders in the muted colour and its name in the normal one; a long path loses its start, never its name. */
function Path({ path }: { path: string }) {
  // A path in the engine's folder reads with "/"; a whole path stays as the engine gave it.
  const whole = /^(?:[A-Za-z]:|[\\/])/.test(path);
  const shown = whole ? path : path.replaceAll('\\', '/');
  const cut = Math.max(shown.lastIndexOf('/'), shown.lastIndexOf('\\')) + 1;
  return (
    <span className="files-card-path">
      {cut > 0 && (
        <span className="files-card-dir">
          <bdi>{shown.slice(0, cut)}</bdi>
        </span>
      )}
      <span className="files-card-name">{shown.slice(cut)}</span>
    </span>
  );
}
