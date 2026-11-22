import { api } from '../api';
import { FieldDialog } from './FieldDialog';

interface Props {
  chat: { id: string; title: string };
  /** The More menu's button, which gets focus back. */
  opener: HTMLElement | null;
  onClose: () => void;
}

/** Rename chat: the person's name for it, which the background model never replaces. */
export function RenameChat({ chat, opener, onClose }: Props) {
  return (
    <FieldDialog
      title="Rename chat"
      label="Name"
      verb="Rename"
      initial={chat.title}
      maxLength={60}
      opener={opener}
      onClose={onClose}
      onSave={(title) => api.renameChat(chat.id, title)}
    />
  );
}
