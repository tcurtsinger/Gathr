import { CircleArrowUp, House, LoaderCircle, Settings } from 'lucide-react';
import { IconButton } from './components/IconButton';
import './rail.css';

interface Props {
  onSettings: (opener: HTMLElement) => void;
  /** Gathr's own update (0032): the version ready to install, or that it's installing now. */
  update: { ready: string | null; updating: boolean; asking: boolean; onUpdate: () => void };
}

/**
 * The rail on the window's shell (0070): where you are at the top, Update and Settings at the foot. The
 * window has one place, so Home is a mark of it, not a button.
 */
export function Rail({ onSettings, update }: Props) {
  return (
    <div className="rail">
      <span className="rail-mark" role="img" aria-label="Home">
        <House aria-hidden="true" />
      </span>
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
