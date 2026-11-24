import { FolderOpen } from 'lucide-react';
import { useLayoutEffect, useRef, useState } from 'react';
import { desktop, pickFolder } from '../desktop';
import { report } from '../report';
import './folder-field.css';

interface Props {
  id: string;
  value: string;
  /** The system picker's title. */
  title: string;
  onChange: (folder: string) => void;
  /** A folder settled on: picked, or typed and left (Enter or leaving the field). */
  onCommit?: (folder: string) => void;
}

/**
 * A project's folder: in the desktop window a field that opens the system's folder picker, showing the folder
 * picked; in a browser, or when the picker fails, a field to type the path.
 */
export function FolderField({ id, value, title, onChange, onCommit }: Props) {
  const [typing, setTyping] = useState(() => !desktop());
  const picking = useRef(false);
  // Away from the field, the path shows its end, which tells folders apart.
  const field = useRef<HTMLInputElement>(null);
  const toEnd = () => {
    const input = field.current;
    if (input && document.activeElement !== input) input.scrollLeft = input.scrollWidth;
  };
  // biome-ignore lint/correctness/useExhaustiveDependencies: scrolls again for each new path
  useLayoutEffect(toEnd, [value, typing]);

  if (typing) {
    return (
      <input
        ref={field}
        id={id}
        className="project-folder"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        onBlur={(e) => {
          onCommit?.(e.target.value);
          toEnd();
        }}
        onKeyDown={(e) => e.key === 'Enter' && onCommit?.(e.currentTarget.value)}
        placeholder="Folder path"
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
        (picked) => {
          if (!picked) return;
          onChange(picked);
          onCommit?.(picked);
        },
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
      <span className={value ? 'folder-path path-tail' : 'folder-path'}>
        <span>{value || 'Choose a folder'}</span>
      </span>
    </button>
  );
}
