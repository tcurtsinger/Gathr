import { ChevronLeft, Search } from 'lucide-react';
import { useEffect, useState } from 'react';
import { Button } from '@/components/ui/button';
import { api, type Design, type DesignPart, type Failure } from '../api';
import { useCard } from '../components/Corner';
import { DesignsMark } from '../components/DesignsMark';
import { PageFrame, PagePreview } from '../room/PageFrame';
import { Screen } from '../Screen';
import './designs.css';

const nameOf = (part: DesignPart) => part.project?.name ?? part.folder;
const letter = (i: number) => String.fromCharCode(65 + i);

/** "3 options · 2h", or "1 page · Yesterday". */
function facts(d: Design, now = Date.now()) {
  const pages = d.pages.length === 1 ? '1 page' : `${d.pages.length} options`;
  return `${pages} · ${ago(d.modified, now)}`;
}

/** A short time since: 40m, 2h today, then Yesterday, 3d this week, else the date. */
function ago(time: number, now: number) {
  const minutes = Math.floor((now - time) / 60_000);
  const day = (t: number) => new Date(new Date(t).toDateString()).getTime();
  const days = Math.round((day(now) - day(time)) / 86_400_000);
  if (minutes < 1) return 'Now';
  if (minutes < 60) return `${minutes}m`;
  if (days <= 0) return `${Math.floor(minutes / 60)}h`;
  if (days === 1) return 'Yesterday';
  if (days < 7) return `${days}d`;
  return new Date(time).toLocaleDateString([], { day: 'numeric', month: 'short' });
}

/**
 * The Designs screen, in place of the chat list and the chat: every design by project, each a card with its first
 * page as it looks, searched by name. A card opens the design large here, with its options and Open in chat.
 */
export function Designs({ onChat }: { onChat: (chat: string, design: Design, page: string) => void }) {
  const [parts, setParts] = useState<DesignPart[]>();
  const [failure, setFailure] = useState<Failure>();
  useCard(failure);
  // A part's folder, or every part.
  const [picked, setPicked] = useState<string>();
  const [query, setQuery] = useState('');
  const [open, setOpen] = useState<{ part: DesignPart; design: Design }>();
  useEffect(() => {
    api.designs().then(setParts, setFailure);
  }, []);
  const pick = (folder?: string) => {
    setPicked(folder);
    setOpen(undefined);
  };
  const words = query.trim().toLowerCase();
  const shown = (parts ?? [])
    .filter((p) => !picked || p.folder === picked)
    .map((p) => ({ ...p, designs: p.designs.filter((d) => d.name.toLowerCase().includes(words)) }))
    .filter((p) => p.designs.length > 0);
  const part = parts?.find((p) => p.folder === picked);

  return (
    <Screen
      name="Designs"
      nav={
        <>
          <div className="sidebar-actions">
            <button
              type="button"
              className="navbutton"
              aria-current={!picked ? 'page' : undefined}
              onClick={() => pick()}
            >
              <DesignsMark />
              All designs
            </button>
          </div>
          {parts && parts.length > 0 && (
            <div className="chatlist">
              <section>
                <h2>Projects</h2>
                {parts.map((p) => (
                  <div key={p.folder} className="chatrow" data-current={p.folder === picked || undefined}>
                    <button
                      type="button"
                      className="chat-link"
                      aria-current={p.folder === picked ? 'page' : undefined}
                      onClick={() => pick(p.folder)}
                    >
                      <span>{nameOf(p)}</span>
                      <span className="nav-count designs-count">{p.designs.length}</span>
                    </button>
                  </div>
                ))}
              </section>
            </div>
          )}
        </>
      }
    >
      {open ? (
        <Opened
          key={open.design.path}
          part={open.part}
          design={open.design}
          onBack={() => setOpen(undefined)}
          onChat={onChat}
        />
      ) : (
        <>
          <h1>{part ? nameOf(part) : 'Designs'}</h1>
          {parts?.length === 0 ? (
            <div className="screen-empty">
              <h2>No designs yet</h2>
              <p className="muted">When an agent draws a design in a chat, it shows here.</p>
            </div>
          ) : (
            parts && (
              <>
                <div className="screen-bar">
                  <label className="search-box designs-search">
                    <Search aria-hidden="true" />
                    <input
                      type="text"
                      placeholder="Search"
                      aria-label="Search designs"
                      value={query}
                      onChange={(e) => setQuery(e.target.value)}
                    />
                  </label>
                </div>
                {shown.map((p) => (
                  <section key={p.folder} className="screen-group designs-group">
                    <h2>
                      {nameOf(p)} <span>{p.designs.length}</span>
                    </h2>
                    <ul className="designs-grid">
                      {p.designs.map((d) => (
                        <Card key={d.path} design={d} onOpen={() => setOpen({ part: p, design: d })} />
                      ))}
                    </ul>
                  </section>
                ))}
              </>
            )
          )}
        </>
      )}
    </Screen>
  );
}

/** A design's card: its first page as it looks, small. The page takes no clicks: the card does. */
function Card({ design, onOpen }: { design: Design; onOpen: () => void }) {
  return (
    <li className="designs-card">
      <PagePreview className="designs-preview" path={design.pages[0]} name={design.name} />
      <button type="button" onClick={onOpen}>
        {design.name}
      </button>
      <span className="muted">{facts(design)}</span>
    </li>
  );
}

interface OpenedProps {
  part: DesignPart;
  design: Design;
  onBack: () => void;
  onChat: (chat: string, design: Design, page: string) => void;
}

/** A design opened from its card: its name and facts, A, B, C for its options, its page as it looks. */
function Opened({ part, design, onBack, onChat }: OpenedProps) {
  const [at, setAt] = useState(0);
  const shown = Math.min(at, design.pages.length - 1);
  const page = design.pages[shown];
  const { chat } = part;
  return (
    <>
      <button type="button" className="screen-back" onClick={onBack}>
        <ChevronLeft aria-hidden="true" />
        Designs
      </button>
      <div className="screen-head">
        <div className="screen-title">
          <h1>{design.name}</h1>
          <p className="muted">
            {nameOf(part)} · {facts(design)}
          </p>
        </div>
        <div className="screen-actions">
          {design.pages.length > 1 && (
            <fieldset className="plugins-kind design-options" aria-label="Options">
              {design.pages.map((p, i) => (
                <button key={p} type="button" aria-pressed={i === shown} onClick={() => setAt(i)}>
                  {letter(i)}
                </button>
              ))}
            </fieldset>
          )}
          {chat && (
            <Button type="button" variant="outline" size="sm" onClick={() => onChat(chat, design, page)}>
              Open in chat
            </Button>
          )}
        </div>
      </div>
      <div className="designs-page">
        <PageFrame key={page} path={page} name={design.name} />
      </div>
    </>
  );
}
