import { File as FileIcon, X } from 'lucide-react';
import { type ReactNode, useContext, useEffect, useRef, useState } from 'react';
import { PagePreview } from '../room/PageFrame';
import { Dialog } from './Dialog';
import { fileName, isPicture, OpenFile } from './files';
import { IconButton } from './IconButton';
import './files.css';

/** A picture from one of the chat's folders, as the service gives it to the window. */
export const pictureUrl = (chat: string, path: string) =>
  `/api/chats/${chat}/picture?path=${encodeURIComponent(path)}`;

interface Props {
  name: string;
  /** The picture to show: none for another kind of file. */
  src?: string;
  onRemove?: () => void;
  /** Opens a file that isn't a picture, in the chat's panel. */
  onOpen?: () => void;
  /** A design's page as it looks, small, in place of the file icon. */
  preview?: ReactNode;
}

/**
 * The one tile a file shows as, everywhere: a square, a picture cropped to fill it, or a file icon with the name along
 * its foot. A click on a picture opens it large, on another file opens it in the chat's panel; a picture that can't be
 * loaded shows as a file. A design's page shows small behind its name: the page takes no clicks, the tile does.
 */
export function FileTile({ name, src, onRemove, onOpen, preview }: Props) {
  const [broken, setBroken] = useState(false);
  const [open, setOpen] = useState(false);
  const tile = useRef<HTMLButtonElement>(null);
  const shown = src && !broken ? src : undefined;
  const close = () => {
    setOpen(false);
    tile.current?.focus();
  };
  return (
    <div className="file-tile">
      {shown ? (
        <button
          ref={tile}
          type="button"
          className="file-face"
          aria-label={`Open ${name}`}
          onClick={() => setOpen(true)}
        >
          <img src={shown} alt={name} onError={() => setBroken(true)} />
        </button>
      ) : preview ? (
        <div className="file-face">
          {preview}
          <button type="button" className="file-over" aria-label={`Open ${name}`} onClick={onOpen}>
            <span className="file-name">{name}</span>
          </button>
        </div>
      ) : onOpen ? (
        <button type="button" className="file-face" aria-label={`Open ${name}`} onClick={onOpen}>
          <FileIcon aria-hidden="true" />
          <span className="file-name">{name}</span>
        </button>
      ) : (
        <div className="file-face" role="img" aria-label={name}>
          <FileIcon aria-hidden="true" />
          <span className="file-name">{name}</span>
        </div>
      )}
      {onRemove && (
        <IconButton aria-label={`Remove ${name}`} className="file-remove" onClick={onRemove}>
          <X aria-hidden="true" />
        </IconButton>
      )}
      {open && shown && (
        <Dialog open title={name} onClose={close} className="file-view" noHeader bare>
          {/* biome-ignore lint/a11y/noStaticElementInteractions: a click beside the picture closes it; Escape does too */}
          {/* biome-ignore lint/a11y/useKeyWithClickEvents: Escape closes it, through the dialog */}
          <div className="file-stage" onClick={(e) => e.target === e.currentTarget && close()}>
            <figure className="file-large">
              <figcaption>
                <span>{name}</span>
                <IconButton aria-label={`Close ${name}`} onClick={close}>
                  <X aria-hidden="true" />
                </IconButton>
              </figcaption>
              <img src={shown} alt={name} />
            </figure>
          </div>
        </Dialog>
      )}
    </div>
  );
}

/** A row of tiles that wraps. */
export function FileTiles({ children }: { children: ReactNode }) {
  return <div className="file-tiles">{children}</div>;
}

/** Tiles for paths in a chat's message: each picture served from the chat's folders. */
export function PathTiles({ chat, paths }: { chat: string; paths: string[] }) {
  const open = useContext(OpenFile);
  return (
    <FileTiles>
      {paths.map((p, i) =>
        isPicture(p) ? (
          // biome-ignore lint/suspicious/noArrayIndexKey: the same file may be named twice in one message
          <FileTile key={i} name={fileName(p)} src={pictureUrl(chat, p)} />
        ) : DESIGN_PAGE.test(p) ? (
          // biome-ignore lint/suspicious/noArrayIndexKey: the same file may be named twice in one message
          <PageTile key={i} path={p} onOpen={open && (() => open(p))} />
        ) : (
          // biome-ignore lint/suspicious/noArrayIndexKey: the same file may be named twice in one message
          <FileTile key={i} name={fileName(p)} onOpen={open && (() => open(p))} />
        ),
      )}
    </FileTiles>
  );
}

/**
 * A full path that looks like a design's page: `…/Designs/<part>/<design>/<page>.html`. Only these ask the service for
 * a small page; the service says whether it is one, and any other page is a file.
 */
const DESIGN_PAGE = /^([A-Za-z]:|\/).*[\\/]Designs[\\/][^\\/]+[\\/][^\\/]+[\\/][^\\/]+\.html?$/i;

/** A design's page: as it looks, small; as a file when the service won't show it. */
function PageTile({ path, onOpen }: { path: string; onOpen?: () => void }) {
  const [broken, setBroken] = useState(false);
  const name = fileName(path);
  const preview = !broken && <PagePreview path={path} name={name} onFail={() => setBroken(true)} />;
  return <FileTile name={name} onOpen={onOpen} preview={preview || undefined} />;
}

/** A file still in the window, not sent yet: a picture shows from memory. */
export function LocalTile({ file, onRemove }: { file: File; onRemove?: () => void }) {
  const [src, setSrc] = useState<string>();
  useEffect(() => {
    if (!isPicture(file.name)) return;
    const url = URL.createObjectURL(file);
    setSrc(url);
    return () => URL.revokeObjectURL(url);
  }, [file]);
  return <FileTile name={file.name} src={src} onRemove={onRemove} />;
}
