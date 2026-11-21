import { BookOpen, Plus, Search } from 'lucide-react';
import { useCallback, useEffect, useRef, useState } from 'react';
import { Button } from '@/components/ui/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { api, type Failure, type Skill } from '../api';
import { Confirm } from '../components/Confirm';
import { IconButton } from '../components/IconButton';
import { FailureNotice } from '../components/Notice';
import { ResizeHandle } from '../components/ResizeHandle';
import { listen } from '../live';
import { TopLine } from '../TopLine';
import { NewSkill } from './NewSkill';
import { SkillMenu } from './SkillMenu';
import { SkillPage } from './SkillPage';
import './plugins.css';

/** The list, a skill's page, or the New skill form. */
type View = { page: 'list' } | { page: 'skill'; name: string } | { page: 'new' };

interface Props {
  onReference: (ref: string) => void;
}

/**
 * The Plugins screen, in place of the chat list and the chat: its sidebar names every skill installed, the main
 * panel lists them, with one search for both. Skills are the only kind there is yet.
 */
export function Plugins({ onReference }: Props) {
  const [skills, setSkills] = useState<Skill[] | null>(null);
  const [view, setView] = useState<View>({ page: 'list' });
  const [query, setQuery] = useState('');
  const [failure, setFailure] = useState<Failure | null>(null);
  const [removing, setRemoving] = useState<{ skill: Skill; opener: HTMLElement | null } | null>(null);
  const search = useRef<HTMLInputElement>(null);

  const load = useCallback(() => api.skills().then((s) => setSkills(s.skills), setFailure), []);
  useEffect(() => {
    load();
    return listen((event) => {
      if (event.type === 'skills' || event.type === 'lagged') load();
    });
  }, [load]);

  const words = query.trim().toLowerCase();
  const shown = (skills ?? []).filter(
    (s) => !words || `${s.name} ${s.description ?? ''}`.toLowerCase().includes(words),
  );
  const open = (name: string) => {
    setFailure(null);
    setView({ page: 'skill', name });
  };
  const acted = (done: Promise<unknown>) => {
    setFailure(null);
    done.catch(setFailure);
  };
  const enable = (skill: Skill, on: boolean) => acted(api.enableSkill(skill.name, on));
  const remove = (skill: Skill, opener: HTMLElement | null) => setRemoving({ skill, opener });
  const added = (done: Promise<string>) => {
    setFailure(null);
    done.then((name) => load().then(() => open(name)), setFailure);
  };

  const row = (s: Skill) => (
    <li key={s.name} className="plugin-row" data-disabled={!s.enabled || undefined}>
      <button type="button" className="plugin-open" onClick={() => open(s.name)}>
        <span className="plugin-tile" aria-hidden="true">
          <BookOpen />
        </span>
        <span className="plugin-words">
          <strong>{s.name}</strong>
          {s.problem ? <span className="plugin-problem">{s.problem}</span> : <span>{s.description}</span>}
        </span>
      </button>
      <SkillMenu
        skill={s}
        onOpen={() => acted(api.openSkill(s.name))}
        onEnable={(on) => enable(s, on)}
        onRemove={(opener) => remove(s, opener)}
      />
    </li>
  );
  const group = (title: string, list: Skill[]) =>
    list.length > 0 && (
      <section className="plugin-group">
        <h2>
          {title} <span className="nav-count">{list.length}</span>
        </h2>
        <ul>{list.map(row)}</ul>
      </section>
    );
  const picked = view.page === 'skill' ? view.name : null;
  // The skill whose page is open, as listed: one gone from the folder (removed here or by hand) goes back to the list.
  const page = skills?.find((s) => s.name === picked);
  useEffect(() => {
    if (picked && skills && !skills.some((s) => s.name === picked)) setView({ page: 'list' });
  }, [picked, skills]);

  return (
    <>
      <nav className="sidebar plugins-nav" aria-label="Plugins">
        <div className="brand">
          <strong>Plugins</strong>
          <IconButton
            aria-label="Search"
            className="row-icon plugins-search-button"
            onClick={() => {
              setView({ page: 'list' });
              requestAnimationFrame(() => search.current?.focus());
            }}
          >
            <Search aria-hidden="true" />
          </IconButton>
        </div>
        <div className="sidebar-actions">
          <button
            type="button"
            className="navbutton"
            aria-current={view.page === 'list' ? 'page' : undefined}
            onClick={() => setView({ page: 'list' })}
          >
            <BookOpen aria-hidden="true" />
            Skills
          </button>
        </div>
        <div className="chatlist">
          <section>
            <h2>Installed</h2>
            {skills?.length === 0 && <p className="sidebar-empty">Nothing yet</p>}
            {shown.map((s) => (
              <div
                key={s.name}
                className="chatrow"
                data-current={s.name === picked || undefined}
                data-disabled={!s.enabled || undefined}
              >
                <button
                  type="button"
                  className="chat-link"
                  aria-current={s.name === picked ? 'page' : undefined}
                  onClick={() => open(s.name)}
                >
                  <BookOpen aria-hidden="true" className="plugins-nav-icon" />
                  <span>{s.name}</span>
                  {s.problem && <i className="plugins-dot" role="img" aria-label="Can't be read" />}
                </button>
              </div>
            ))}
          </section>
        </div>
        <ResizeHandle
          label="Resize navigation"
          side="right"
          name="--sidebar-width"
          min={190}
          max={() => 400}
        />
      </nav>
      <TopLine />
      <main className="chat plugins" aria-label="Plugins">
        <div className="plugins-page">
          {failure && <FailureNotice failure={failure} onReference={onReference} />}
          {page && (
            <SkillPage
              key={page.name}
              skill={page}
              onBack={() => setView({ page: 'list' })}
              onFailure={setFailure}
              onEnable={enable}
              onRemove={remove}
            />
          )}
          {view.page === 'new' && (
            <NewSkill
              onBack={() => setView({ page: 'list' })}
              onSave={(skill) => api.newSkill(skill).then((s) => load().then(() => open(s.name)))}
              onReference={onReference}
            />
          )}
          {view.page === 'list' && (
            <>
              <h1>Skills</h1>
              <div className="plugins-bar">
                {skills?.length !== 0 && (
                  <label className="plugins-search">
                    <Search aria-hidden="true" />
                    <input
                      ref={search}
                      type="text"
                      placeholder="Search"
                      aria-label="Search skills"
                      value={query}
                      onChange={(e) => setQuery(e.target.value)}
                    />
                  </label>
                )}
                <AddMenu
                  onFolder={(files) => added(api.addSkillFolder(files))}
                  onFile={(file) => added(api.addSkillFile(file))}
                  onNew={() => setView({ page: 'new' })}
                />
              </div>
              {skills?.length === 0 ? (
                <div className="plugins-empty">
                  <h2>Nothing installed yet</h2>
                  <p className="muted">
                    A skill is a folder of instructions an agent reads when a task needs them.
                  </p>
                </div>
              ) : (
                <>
                  {group(
                    'Skills',
                    shown.filter((s) => s.enabled),
                  )}
                  {group(
                    'Disabled',
                    shown.filter((s) => !s.enabled),
                  )}
                </>
              )}
            </>
          )}
        </div>
      </main>
      {removing && (
        <Confirm
          question={`Remove ${removing.skill.name}?`}
          body={`Its folder is deleted, with all its files: ${removing.skill.path}`}
          verb="Remove"
          danger
          opener={removing.opener}
          onCancel={() => setRemoving(null)}
          onConfirm={() =>
            api.removeSkill(removing.skill.name).then(() => {
              setRemoving(null);
              setView({ page: 'list' });
            })
          }
          onReference={onReference}
        />
      )}
    </>
  );
}

interface AddProps {
  onFolder: (files: File[]) => void;
  onFile: (file: File) => void;
  onNew: () => void;
}

/** Add: a skill from a folder, from one Markdown file, or written here. */
function AddMenu({ onFolder, onFile, onNew }: AddProps) {
  const folder = useRef<HTMLInputElement>(null);
  const file = useRef<HTMLInputElement>(null);
  const picked = (input: HTMLInputElement, take: (files: File[]) => void) => {
    const files = [...(input.files ?? [])];
    input.value = '';
    if (files.length) take(files);
  };
  return (
    <>
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button type="button" variant="outline" size="sm" className="plugins-add">
            <Plus aria-hidden="true" />
            Add
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" sideOffset={6}>
          <DropdownMenuItem onSelect={() => folder.current?.click()}>From a folder</DropdownMenuItem>
          <DropdownMenuItem onSelect={() => file.current?.click()}>From a file</DropdownMenuItem>
          <DropdownMenuItem onSelect={onNew}>New skill</DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
      <input
        ref={(input) => {
          folder.current = input;
          // React passes no `webkitdirectory` prop through; the attribute turns the picker to folders.
          input?.setAttribute('webkitdirectory', '');
        }}
        type="file"
        hidden
        onChange={(e) => picked(e.currentTarget, onFolder)}
      />
      <input
        ref={file}
        type="file"
        accept=".md"
        hidden
        onChange={(e) => picked(e.currentTarget, ([f]) => onFile(f))}
      />
    </>
  );
}
