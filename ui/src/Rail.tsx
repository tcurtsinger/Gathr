import { AtSign, CircleArrowUp, House, LoaderCircle, Settings } from 'lucide-react';
import { IconButton } from './components/IconButton';
import './rail.css';

interface Props {
  /** Whether the Plugins screen is open: its icon is lit, else Home's. */
  plugins: boolean;
  onHome: () => void;
  onPlugins: () => void;
  onSettings: (opener: HTMLElement) => void;
  /** Gathr's own update (0032): the version ready to install, or that it's installing now. */
  update: { ready: string | null; updating: boolean; asking: boolean; onUpdate: () => void };
}

/** The rail on the window's shell (0070): Home and Plugins at the top, the open one lit; Update and Settings at the foot. */
export function Rail({ plugins, onHome, onPlugins, onSettings, update }: Props) {
  return (
    <div className="rail">
      <IconButton
        aria-label="Home"
        aria-current={plugins ? undefined : 'page'}
        className="rail-button"
        onClick={onHome}
      >
        <House aria-hidden="true" />
      </IconButton>
      <IconButton
        aria-label="Plugins"
        aria-current={plugins ? 'page' : undefined}
        className="rail-button"
        onClick={onPlugins}
      >
        <AtSign aria-hidden="true" />
      </IconButton>
      <div className="rail-foot">
        {update.ready && (
          <IconButton
            aria-label="Update"
            className="rail-button rail-update"
            disabled={update.asking}
            onClick={update.onUpdate}
          >
            <CircleArrowUp aria-hidden="true" />
          </IconButton>
        )}
        {update.updating && (
          <span className="rail-button rail-updating" role="status" aria-label="Updating">
            <LoaderCircle aria-hidden="true" />
          </span>
        )}
        <IconButton
          aria-label="Settings"
          className="rail-button"
          onClick={(e) => onSettings(e.currentTarget)}
        >
          <Settings aria-hidden="true" />
        </IconButton>
      </div>
    </div>
  );
}
