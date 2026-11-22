import { ChevronRight, Folder } from 'lucide-react';
import { type CSSProperties, useEffect, useRef, useState } from 'react';
import { api, type ChangedFile, type Changes as ChatChanges, type Failure, type FileChange } from '../api';
import { ChangesMark } from '../components/ChangesMark';
import { useCard } from '../components/Corner';
import { Face } from '../components/Face';
import { fileName } from '../components/files';
import { useLook } from './Design';
import type { RoomState } from './useRoom';
import './changes.css';

/** A change of at most this many lines starts with every file open; a larger one with every file shut. */
const OPEN_UP_TO = 600;

const count = (n: number, one: string) => `${n.toLocaleString('en')} ${n === 1 ? one : `${one}s`}`;
const sum = (files: ChangedFile[]) => ({
  added: files.reduce((n, f) => n + f.added, 0),
  removed: files.reduce((n, f) => n + f.removed, 0),
});

/** Lines added and removed, each when there are any. */
function Totals({ added, removed }: { added: number; removed: number }) {
  return (
    <span className="changes-totals">
      {added > 0 && <span className="changes-plus">+{added}</span>}
      {removed > 0 && <span className="changes-minus">−{removed}</span>}
    </span>
  );
}

// Each chat's last read change, while the window is open: the tab opened from Home's tile shows it at once, and its
// own read replaces it.
const last = new Map<string, ChatChanges>();

/** The chat's change, read on each look while `open`: the last read until then, undefined before any. */
function useChanges(chat: string, room: RoomState, open: boolean) {
  const [changes, setChanges] = useState(() => last.get(chat));
  const [failure, setFailure] = useState<Failure>();
  useCard(failure);
  const look = useLook(room, open);
  useEffect(() => {
    if (!look) return;
    let live = true;
    api.changes(chat).then(
      (c) => {
        last.set(chat, c);
        if (live) setChanges(c);
      },
      (f: Failure) => live && setFailure(f),
    );
    return () => {
      live = false;
    };
  }, [chat, look]);
  return changes;
}

/** Every changed file of an agent: in its worktree, then in each other checkout. */
const filesOf = (a: ChatChanges['agents'][number]) => [...a.files, ...a.outside.flatMap((o) => o.files)];

/** Home's Changes tile: how many files the chat's agents changed, once read, when they changed any. */
export function ChangesTile({ chat, room, open, onOpen }: Props & { onOpen: () => void }) {
  const changes = useChanges(chat, room, open);
  const files = changes?.agents.reduce((n, a) => n + filesOf(a).length, 0) ?? 0;
  return (
    <button type="button" className="home-tile" onClick={onOpen}>
      <ChangesMark />
      Changes
      {files > 0 && <span className="changes-count">{files}</span>}
    </button>
  );
}

interface Props {
  chat: string;
  room: RoomState;
  /** Whether it is the shown tab: only then it reads the change again. */
  open: boolean;
}

/**
 * The Changes tab: every file each agent changed, one under another by agent, each folding open or shut: in its
 * worktree, then under each other checkout it wrote in. A file's lines are read when it is first open, and again when
 * its totals change.
 */
export function Changes({ chat, room, open }: Props) {
  const changes = useChanges(chat, room, open);
  // Each file's fold, by agent, checkout and path, kept from when it is first seen: as the whole change's size says then.
  const folds = useRef(new Map<string, boolean>()).current;
  const [, setFolded] = useState(0);
  if (!changes) return null;
  const agents = changes.agents.filter((a) => filesOf(a).length > 0);
  const all = agents.flatMap(filesOf);
  if (!all.length)
    return (
      <div className="filetab-none">
        <ChangesMark />
        <strong>No changes</strong>
      </div>
    );
  const total = sum(all);
  const starts = total.added + total.removed <= OPEN_UP_TO;
  const filesIn = (agent: string, files: ChangedFile[], folder?: string) =>
    files.map((f) => {
      const key = `${agent}\n${folder ?? ''}\n${f.path}`;
      if (!folds.has(key)) folds.set(key, starts);
      const shown = folds.get(key) ?? starts;
      return (
        <FileChangeView
          key={key}
          chat={chat}
          agent={agent}
          folder={folder}
          file={f}
          open={shown}
          onFold={() => {
            folds.set(key, !shown);
            setFolded((n) => n + 1);
          }}
        />
      );
    });
  return (
    <div className="changes">
      <div className="changes-bar">
        <strong>{count(all.length, 'file')} changed</strong>
        <Totals {...total} />
      </div>
      <div className="changes-scroll">
        {agents.map((a) => (
          <section key={a.id}>
            <div className="changes-agent">
              <Face agent={{ id: a.id, name: a.name }} size={16} />
              <strong>{a.name}</strong>
              <span className="changes-branch">{a.branch}</span>
              <span className="changes-files">{count(filesOf(a).length, 'file')}</span>
              <Totals {...sum(filesOf(a))} />
            </div>
            {filesIn(a.id, a.files)}
            {a.outside.map((o) => (
              <div key={o.folder}>
                <div className="changes-agent changes-folder">
                  <Folder aria-hidden="true" />
                  <strong>{fileName(o.folder)}</strong>
                  <span className="changes-branch">{o.folder}</span>
                </div>
                {filesIn(a.id, o.files, o.folder)}
              </div>
            ))}
          </section>
        ))}
        {changes.more && <p className="changes-note">First {count(all.length, 'file')}</p>}
      </div>
    </div>
  );
}

interface FileProps {
  chat: string;
  agent: string;
  /** The checkout outside the chat's worktrees it is in. */
  folder?: string;
  file: ChangedFile;
  open: boolean;
  onFold: () => void;
}

/** One file: its head, which folds it, and under it its lines while it is open. */
function FileChangeView({ chat, agent, folder, file, open, onFold }: FileProps) {
  const cut = file.path.lastIndexOf('/') + 1;
  const pill = file.kind === 'new' ? 'New' : file.kind === 'deleted' ? 'Deleted' : undefined;
  return (
    <div className="changes-file">
      <button type="button" className="changes-head" aria-expanded={open} onClick={onFold}>
        <ChevronRight aria-hidden="true" data-open={open} />
        <span className="changes-path">
          <bdi>
            {file.path.slice(0, cut)}
            <strong>{file.path.slice(cut)}</strong>
          </bdi>
        </span>
        {pill && <span className={`changes-pill ${file.kind}`}>{pill}</span>}
        <Totals added={file.added} removed={file.removed} />
      </button>
      {open &&
        (file.binary ? (
          <p className="changes-note">Binary file</p>
        ) : (
          <Lines chat={chat} agent={agent} folder={folder} file={file} />
        ))}
    </div>
  );
}

/** A file's lines, read again when its totals change. */
function Lines({ chat, agent, folder, file }: Omit<FileProps, 'open' | 'onFold'>) {
  const [got, setGot] = useState<{ said: string; change: FileChange }>();
  const [failure, setFailure] = useState<Failure>();
  useCard(failure);
  const said = `${file.kind} ${file.added} ${file.removed}`;
  // biome-ignore lint/correctness/useExhaustiveDependencies: read when first open and when its totals change
  useEffect(() => {
    if (got?.said === said) return;
    let live = true;
    api.change(chat, agent, file, folder).then(
      (change) => live && setGot({ said, change }),
      (f: Failure) => live && setFailure(f),
    );
    return () => {
      live = false;
    };
  }, [chat, agent, folder, file.path, said]);
  if (!got) return null;
  const { change } = got;
  if (change.binary) return <p className="changes-note">Binary file</p>;
  const widest = Math.max(1, ...change.hunks.map((h) => Math.max(h.old[0] + h.old[1], h.new[0] + h.new[1])));
  return (
    <>
      {change.cut && <p className="changes-note">First 2,000 lines</p>}
      <div className="changes-lines" style={{ '--digits': String(widest).length } as CSSProperties}>
        <div>
          {change.hunks.map((h, i) => (
            // biome-ignore lint/suspicious/noArrayIndexKey: two edits of one file can start at the same lines
            <div key={i}>
              {(i > 0 || h.old[0] > 1 || h.new[0] > 1) && (
                <div className="changes-line changes-gap">
                  <span />
                  <span />
                  <span>
                    Lines {h.new[0]}–{h.new[0] + Math.max(h.new[1], 1) - 1}
                  </span>
                </div>
              )}
              {h.lines.map((l, n) => (
                // biome-ignore lint/suspicious/noArrayIndexKey: a hunk's lines are its own, in order
                <div key={n} className="changes-line" data-kind={l.kind}>
                  <span>{l.old}</span>
                  <span>{l.new}</span>
                  <span>{l.kind === 'added' ? '+' : l.kind === 'removed' ? '−' : ''}</span>
                  <span>{l.text}</span>
                </div>
              ))}
            </div>
          ))}
        </div>
      </div>
    </>
  );
}
