import { api, type WebTool } from '../api';
import { FieldDialog } from '../components/FieldDialog';

interface Props {
  tool: WebTool;
  opener: HTMLElement | null;
  onClose: () => void;
}

/**
 * Change key (Add key for a tool an agent added without one): one password field. Gathr tries the key first and saves
 * it only when the tool takes it.
 */
export function ChangeKey({ tool, opener, onClose }: Props) {
  return (
    <FieldDialog
      title={tool.has_key ? 'Change key' : 'Add key'}
      label="Key"
      verb="Save"
      password
      note="The key stays in Gathr. Agents never see it."
      opener={opener}
      onClose={onClose}
      onSave={(key) => api.changeTool(tool.id, { key })}
    />
  );
}
