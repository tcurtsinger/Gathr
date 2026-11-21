import { ChevronRight, CodeXml, Copy, File as FileIcon, FileX, Folder, FolderOpen } from 'lucide-react';
import { type CSSProperties, useCallback, useContext, useEffect, useState } from 'react';
import { api, Failure, type FileEntry, type FileTree, type Unshown } from '../api';
import { useCard } from '../components/Corner';
import { Face } from '../components/Face';
import { pictureUrl } from '../components/FileTile';
import { isPicture, OpenFile } from '../components/files';
import { IconButton } from '../components/IconButton';
import { FileBody, hasLook } from './FileBody';
import { isPage, PageFrame } from './PageFrame';
import { PdfBody } from './PdfBody';
import { SheetBody } from './SheetBody';

/** A file open in a tab: its path, and its place in the tree from the top folder down. */
export interface Opened {
  path: string;
  crumbs: string[];
  /** A design's folder, shown as a design rather than as a file. */
  design?: boolean;
  /** The design's option shown, by its page's path: the first when unset. */
  page?: string;
}

/** The Files tab: the project's folder, then Shared, the project's Designs and each agent's folder for this chat. */
export function Tree({ chat, onOpen }: { chat: string; onOpen: (file: Opened) => void }) {
  const [tree, setTree] = useState<FileTree>();
  const [failure, setFailure] = useState<Failure>();
  useCard(failure);
  useEffect(() => {
    api.tree(chat).then(setTree, setFailure);
  }, [chat]);
  if (!tree) return null;
  const top = (entry: FileEntry, opened: boolean, agent?: string) => (
    <Node
      key={entry.path}
      chat={chat}
      entry={entry}
      agent={agent}
      opened={opened}
      crumbs={[]}
      onOpen={onOpen}
    />
  );
  const agents = [tree.shared, tree.designs, ...tree.agents].flatMap((p) => (p ? [p] : []));
  return (
    <div className="tree">
      {tree.project && <h3>Project</h3>}
      {tree.project && top({ ...tree.project, folder: true }, true)}
      {agents.length > 0 && <h3>Agents</h3>}
      {agents.map((p) => top({ ...p, folder: true }, p === tree.shared, p.agent))}
      {!tree.project && !agents.length && <p className="tree-empty">No files yet</p>}
    </div>
  );
}

interface NodeProps {
  chat: string;
  entry: FileEntry;
  /** The agent whose folder it is, drawn with its face. */
  agent?: string;
  opened?: boolean;
  crumbs: string[];
  onOpen: (file: Opened) => void;
}

/** A folder, read each time it opens, or a file, which opens in its own tab. */
function Node({ chat, entry, agent, opened = false, crumbs, onOpen }: NodeProps) {
  const [open, setOpen] = useState(opened);
  const [entries, setEntries] = useState<FileEntry[]>();
  const [failure, setFailure] = useState<Failure>();
  useCard(failure);
  useEffect(() => {
    if (!open || !entry.folder) return;
    let live = true;
    api.folder(chat, entry.path).then(
      (e) => live && setEntries(e),
      (f: Failure) => live && setFailure(f),
    );
    return () => {
      live = false;
    };
  }, [open, chat, entry.path, entry.folder]);
  const here = [...crumbs, entry.name];
  const depth = { '--depth': crumbs.length } as CSSProperties;
  const icon = agent ? (
    <Face agent={{ id: agent, name: entry.name }} size={16} />
  ) : entry.folder ? (
    open ? (
      <FolderOpen aria-hidden="true" />
    ) : (
      <Folder aria-hidden="true" />
    )
  ) : (
    <FileIcon aria-hidden="true" />
  );
  return (
    <>
      <button
        type="button"
        className="tree-row"
        style={depth}
        aria-expanded={entry.folder ? open : undefined}
        onClick={() => (entry.folder ? setOpen(!open) : onOpen({ path: entry.path, crumbs: here }))}
      >
        {entry.folder ? (
          <ChevronRight className="tree-chevron" data-open={open} aria-hidden="true" />
        ) : (
          <span className="tree-chevron" />
        )}
        {icon}
        <span>{entry.name}</span>
      </button>
      {open && entries?.map((e) => <Node key={e.path} chat={chat} entry={e} crumbs={here} onOpen={onOpen} />)}
    </>
  );
}

const KINDS: Record<string, string> = {
  doc: 'Word',
  docx: 'Word',
  ppt: 'PowerPoint',
  pptx: 'PowerPoint',
};

/** Files whose tab draws them itself from the service's own answer, not from their words. */
const DOCUMENTS: Record<string, typeof PdfBody> = {
  pdf: PdfBody,
  xlsx: SheetBody,
  xlsm: SheetBody,
  xls: SheetBody,
  ods: SheetBody,
};

/** A file's tab: its path with Source and Copy, and its words as they look, its picture, a PDF's pages or a
 * workbook's sheets. Viewing only. */
export function FileTab({ chat, file }: { chat: string; file: Opened }) {
  const picture = isPicture(file.path);
  const name = file.crumbs.at(-1) ?? file.path;
  const ending = name.split('.').pop()?.toLowerCase() ?? '';
  const Document = DOCUMENTS[ending];
  const [shown, setShown] = useState<{ text: string } | { unshown: Unshown }>();
  const [broken, setBroken] = useState(false);
  const [source, setSource] = useState(false);
  const [failure, setFailure] = useState<Failure>();
  useCard(failure);
  useEffect(() => {
    if (!picture && !Document) api.words(chat, file.path).then(setShown, setFailure);
  }, [chat, file.path, picture, Document]);
  const unshow = useCallback((unshown: Unshown) => setShown({ unshown }), []);
  const text = shown && 'text' in shown ? shown.text : undefined;
  const page = isPage(name);
  const said = broken ? 'gone' : shown && 'unshown' in shown ? shown.unshown : undefined;
  // A page too large to show as text still shows as it looks, by its address; only its Source and Copy go.
  const unshown = page && said !== 'gone' ? undefined : said;
  const kind = KINDS[ending];
  const copy = (words: string) =>
    navigator.clipboard.writeText(words).catch(() => setFailure(new Failure('Copy failed', null, null)));
  // A link in the file that isn't a full path is beside the file, not in an agent's folder.
  const open = useContext(OpenFile);
  const folder = file.path.replace(/[\\/][^\\/]*$/, '');
  const beside = open && ((path: string) => open(/^([A-Za-z]:|\/)/.test(path) ? path : `${folder}/${path}`));
  return (
    <div className="filetab">
      <div className="filetab-path">
        <span>
          {file.crumbs.slice(0, -1).map((c) => `${c} / `)}
          <strong>{name}</strong>
        </span>
        {text !== undefined && (hasLook(name) || page) && (
          <IconButton
            aria-label="Source"
            aria-pressed={source}
            className="filetab-source"
            onClick={() => setSource(!source)}
          >
            <CodeXml />
          </IconButton>
        )}
        {text !== undefined && (
          <IconButton aria-label="Copy" onClick={() => copy(text)}>
            <Copy />
          </IconButton>
        )}
      </div>
      {unshown ? (
        <div className="filetab-none">
          {unshown === 'gone' ? <FileX aria-hidden="true" /> : <FileIcon aria-hidden="true" />}
          <strong>
            {unshown === 'gone'
              ? 'File not found'
              : unshown === 'large'
                ? 'Too large to show'
                : kind
                  ? `Can't show ${kind} files`
                  : "Can't show this file"}
          </strong>
          {unshown === 'gone' && <span>It was moved or deleted.</span>}
        </div>
      ) : Document ? (
        <Document chat={chat} path={file.path} onUnshown={unshow} onFailure={setFailure} />
      ) : page && shown && !source ? (
        <PageFrame chat={chat} path={file.path} name={name} />
      ) : picture ? (
        <div className="filetab-picture">
          <img src={pictureUrl(chat, file.path)} alt={name} onError={() => setBroken(true)} />
        </div>
      ) : (
        text !== undefined && (
          <OpenFile.Provider value={beside}>
            <FileBody name={name} text={text} source={source} />
          </OpenFile.Provider>
        )
      )}
    </div>
  );
}
