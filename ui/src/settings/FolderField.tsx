import { FolderOpen } from 'lucide-react';
import { useRef, useState } from 'react';
import { desktop, pickFolder } from '../desktop';
import { report } from '../report';
import './folder-field.css';

interface Props {
  id: string;
  value: string;
  /** The system picker's title. */
  title: string;
  onChange: (folder: string) => void;
}

/**
 * A project's folder: in the desktop window a field that opens the system's folder picker, showing the folder
 * picked; in a browser, or when the picker fails, a field to type the path.
 */
export function FolderField({ id, value, title, onChange }: Props) {
  const [typing, setTyping] = useState(() => !desktop());
  const picking = useRef(false);

  if (typing) {
    return (
      <input
        id={id}
        className="project-folder"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        required
        spellCheck={false}
        autoComplete="off"
      />
    );
  }

  const pick = () => {
    // One picker at a time: each open is its own system dialog.
    if (picking.current) return;
    picking.current = true;
    pickFolder(title, value || null)
      .then(
        (picked) => picked && onChange(picked),
        (e) => {
          report({ name: 'gathr.ui.window.folder', error: 'open', attrs: { message: String(e) } });
          setTyping(true);
        },
      )
      .finally(() => {
        picking.current = false;
      });
  };

  return (
    <button type="button" id={id} className="folder-field" data-empty={value ? undefined : ''} onClick={pick}>
      <FolderOpen aria-hidden />
      <span className="folder-path">{value || 'Choose a folder'}</span>
    </button>
  );
}
