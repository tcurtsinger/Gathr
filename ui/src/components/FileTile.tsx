import { File as FileIcon, X } from 'lucide-react';
import { type ReactNode, useEffect, useRef, useState } from 'react';
import { Dialog } from './Dialog';
import { fileName, isPicture } from './files';
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
}

/**
 * The one tile a file shows as, everywhere: a square, a picture cropped to fill it, or a file icon with the name along
 * its foot. A click on a picture opens it large; a picture that can't be loaded shows as a file.
 */
export function FileTile({ name, src, onRemove }: Props) {
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
        <Dialog open title={name} onClose={close} className="file-view" closeIcon>
          {/* biome-ignore lint/a11y/noStaticElementInteractions: a click beside the picture closes it; Escape does too */}
          {/* biome-ignore lint/a11y/useKeyWithClickEvents: Escape closes it, through the dialog */}
          <div className="file-stage" onClick={(e) => e.target === e.currentTarget && close()}>
            <img src={shown} alt={name} />
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
  return (
    <FileTiles>
      {paths.map((p, i) => (
        // biome-ignore lint/suspicious/noArrayIndexKey: the same file may be named twice in one message
        <FileTile key={i} name={fileName(p)} src={isPicture(p) ? pictureUrl(chat, p) : undefined} />
      ))}
    </FileTiles>
  );
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
