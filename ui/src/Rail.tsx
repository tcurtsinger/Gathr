import { CircleArrowUp, House, LoaderCircle, Settings } from 'lucide-react';
import type { ReactNode } from 'react';
import { DesignsMark } from './components/DesignsMark';
import { IconButton } from './components/IconButton';
import { MemoryMark } from './components/MemoryMark';
import { EYES, SHAPES } from './components/pictures';
import { ScheduledMark } from './components/ScheduledMark';
import './rail.css';

/** The screen in the frame: the chats (Home), Agents, Plugins, Designs, Memory or Scheduled. */
export type Opened = 'home' | 'agents' | 'plugins' | 'designs' | 'memory' | 'scheduled';

interface Props {
  /** Its icon is lit. */
  open: Opened;
  onOpen: (screen: Opened) => void;
  onSettings: (opener: HTMLElement) => void;
  /** An agent's account is signed out: a dot on Settings. */
  signedOut: boolean;
  /** Gathr's own update (0032): the version ready to install, or that it's installing now. */
  update: { ready: string | null; updating: boolean; asking: boolean; onUpdate: () => void };
}

/**
 * The rail on the window's shell (0070): Home, Agents, Memory, Plugins, Designs and Scheduled at the top, the open one
 * lit; Update and Settings at the foot.
 */
export function Rail({ open, onOpen, onSettings, signedOut, update }: Props) {
  const button = (screen: Opened, name: string, icon: ReactNode) => (
    <IconButton
      aria-label={name}
      aria-current={open === screen ? 'page' : undefined}
      className="rail-button"
      onClick={() => onOpen(screen)}
    >
      {icon}
    </IconButton>
  );
  return (
    <div className="rail">
      {button('home', 'Home', <House aria-hidden="true" />)}
      {button('agents', 'Agents', <AgentsMark />)}
      {button('memory', 'Memory', <MemoryMark />)}
      {button('plugins', 'Plugins', <PlugMark />)}
      {button('designs', 'Designs', <DesignsMark />)}
      {button('scheduled', 'Scheduled', <ScheduledMark />)}
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
          className="rail-button rail-settings"
          data-attention={signedOut || undefined}
          onClick={(e) => onSettings(e.currentTarget)}
        >
          <Settings aria-hidden="true" />
        </IconButton>
      </div>
    </div>
  );
}

const cloud = SHAPES.find((s) => s.key === 'cloud')?.d;
const ovals = EYES.find((e) => e.key === 'ovals')?.d ?? [];

/** Agents' own mark: an agent's picture, the Cloud with the Ovals eyes, scaled to the icon set's grid and line. */
export function AgentsMark() {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <g transform="translate(12 12) scale(0.6) translate(-24 -24.6)" strokeWidth={2.92}>
        <path d={cloud} />
        <g transform="translate(24 24)">
          {ovals.map((d) => (
            <path key={d} d={d} />
          ))}
        </g>
      </g>
    </svg>
  );
}

/** Plugins' own mark, drawn on the icon set's grid: a plug inside an open ring, its cord the ring's tail. */
function PlugMark() {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <g transform="rotate(-45 12 12)">
        <path d="M10.2 5.8v3.2M13.8 5.8v3.2" />
        <path d="M8 9h8v1.5a4 4 0 0 1-8 0z" />
      </g>
      <path d="M13.8 13.8L15 15C17.5 17.5 22 16 22 12a10 10 0 1 0-4 8" />
    </svg>
  );
}
