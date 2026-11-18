import { ComposerMenu, ComposerMenuItem } from '@/components/assistant-ui/elements/composer';
import { Face, type FaceMark } from './Face';

/** Someone "@" can name: `meta` is its role in the chat; `out` when naming it brings it into the chat. */
export interface Person {
  id: string;
  name: string;
  meta?: string;
  out?: boolean;
  mark?: FaceMark;
}

interface Props {
  people: readonly Person[];
  /** The lit row, which Enter picks. */
  lit: number;
  onPick: (index: number) => void;
}

/** The "@" list above the message box: the composer's one menu, a face at 24 with its mark on each row. */
export function MentionList({ people, lit, onPick }: Props) {
  const row = (p: Person) => {
    const k = people.indexOf(p);
    return (
      <ComposerMenuItem
        key={p.id}
        id={`mention-${p.id}`}
        role="option"
        aria-selected={k === lit}
        active={k === lit}
        tabIndex={-1}
        // The keyboard stays in the box.
        onMouseDown={(e) => e.preventDefault()}
        onClick={() => onPick(k)}
      >
        <Face agent={{ id: p.id, name: p.name }} size={24} mark={p.mark} />
        <span className="flex-1 truncate text-start">{p.name}</span>
        {p.meta && <span className="mention-meta">{p.meta}</span>}
      </ComposerMenuItem>
    );
  };
  const outside = people.filter((p) => p.out);
  return (
    <ComposerMenu open role="listbox" aria-label="Agents" className="mention-list">
      {people.filter((p) => !p.out).map(row)}
      {outside.length > 0 && (
        // biome-ignore lint/a11y/useSemanticElements: a listbox's groups, as ARIA's combobox pattern has them
        <div role="group" aria-labelledby="mention-bring-in">
          <div id="mention-bring-in" className="mention-label">
            Bring in
          </div>
          {outside.map(row)}
        </div>
      )}
    </ComposerMenu>
  );
}
