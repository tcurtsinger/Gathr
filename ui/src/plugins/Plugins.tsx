import { BookOpen, Plug, Plus, Search } from 'lucide-react';
import { useCallback, useEffect, useRef, useState } from 'react';
import { Button } from '@/components/ui/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { api, type Failure, type Skill, type WebTool } from '../api';
import { Confirm } from '../components/Confirm';
import { IconButton } from '../components/IconButton';
import { FailureNotice } from '../components/Notice';
import { ResizeHandle } from '../components/ResizeHandle';
import { listen } from '../live';
import { TopLine } from '../TopLine';
import { AddTool } from './AddTool';
import { ChangeKey } from './ChangeKey';
import { NewSkill } from './NewSkill';
import { SkillMenu } from './SkillMenu';
import { SkillPage } from './SkillPage';
import { ToolMenu } from './ToolMenu';
import { ToolPage } from './ToolPage';
import { host, stateOf } from './tool';
import './plugins.css';

/** The list, a skill's or a tool's page, New skill, or Add a tool. */
type View =
  | { page: 'list' }
  | { page: 'skill'; name: string }
  | { page: 'tool'; id: string }
  | { page: 'new' }
  | { page: 'add-tool' };
/** The list's choice: both kinds, or one; the sidebar's Tools and Skills set it too. */
type Kind = 'all' | 'tools' | 'skills';

type Removing = { skill: Skill; opener: HTMLElement | null } | { tool: WebTool; opener: HTMLElement | null };

interface Props {
  onReference: (ref: string) => void;
}

/**
 * The Plugins screen, in place of the chat list and the chat: its sidebar names every tool and skill installed, the
 * main panel lists them in groups, with one search for both.
 */
export function Plugins({ onReference }: Props) {
  const [skills, setSkills] = useState<Skill[] | null>(null);
  const [tools, setTools] = useState<WebTool[] | null>(null);
  const [view, setView] = useState<View>({ page: 'list' });
  const [kind, setKind] = useState<Kind>('all');
  const [query, setQuery] = useState('');
  const [failure, setFailure] = useState<Failure | null>(null);
  const [removing, setRemoving] = useState<Removing | null>(null);
  const [changingKey, setChangingKey] = useState<{ tool: WebTool; opener: HTMLElement | null } | null>(null);
  const search = useRef<HTMLInputElement>(null);

  const loadSkills = useCallback(() => api.skills().then((s) => setSkills(s.skills), setFailure), []);
  const loadTools = useCallback(() => api.tools().then(setTools, setFailure), []);
  useEffect(() => {
    loadSkills();
    loadTools();
    return listen((event) => {
      if (event.type === 'skills' || event.type === 'lagged') loadSkills();
      if (event.type === 'tools' || event.type === 'lagged') loadTools();
    });
  }, [loadSkills, loadTools]);

  const words = query.trim().toLowerCase();
  const found = (text: string) => !words || text.toLowerCase().includes(words);
  const shownSkills =
    kind === 'tools' ? [] : (skills ?? []).filter((s) => found(`${s.name} ${s.description ?? ''}`));
  const shownTools = kind === 'skills' ? [] : (tools ?? []).filter((t) => found(`${t.name} ${host(t.link)}`));
  const nothing = skills?.length === 0 && tools?.length === 0;
  const openSkill = (name: string) => {
    setFailure(null);
    setView({ page: 'skill', name });
  };
  const openTool = (id: string) => {
    setFailure(null);
    setView({ page: 'tool', id });
  };
  const acted = (done: Promise<unknown>) => {
    setFailure(null);
    done.catch(setFailure);
  };
  const enableSkill = (skill: Skill, on: boolean) => acted(api.enableSkill(skill.name, on));
  const enableTool = (tool: WebTool, on: boolean) => acted(api.changeTool(tool.id, { enabled: on }));
  const changeKey = (tool: WebTool, opener: HTMLElement | null) => setChangingKey({ tool, opener });
  const added = (done: Promise<string>) => {
    setFailure(null);
    done.then((name) => loadSkills().then(() => openSkill(name)), setFailure);
  };
  const pick = (next: Kind) => {
    setKind(next);
    setView({ page: 'list' });
  };

  const skillRow = (s: Skill) => (
    <li key={`s-${s.name}`} className="plugin-row" data-disabled={!s.enabled || undefined}>
      <button type="button" className="plugin-open" onClick={() => openSkill(s.name)}>
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
        onEnable={(on) => enableSkill(s, on)}
        onRemove={(opener) => setRemoving({ skill: s, opener })}
      />
    </li>
  );
  const toolRow = (t: WebTool) => {
    const state = stateOf(t);
    return (
      <li key={`t-${t.id}`} className="plugin-row" data-disabled={!t.enabled || undefined}>
        <button type="button" className="plugin-open" onClick={() => openTool(t.id)}>
          <span className="plugin-tile" aria-hidden="true">
            <Plug />
          </span>
          <span className="plugin-words">
            <strong>{t.name}</strong>
            <span>
              {host(t.link)} · <span data-tone={state.tone}>{state.words}</span>
            </span>
          </span>
        </button>
        <ToolMenu
          tool={t}
          onChangeKey={(opener) => changeKey(t, opener)}
          onEnable={(on) => enableTool(t, on)}
          onRemove={(opener) => setRemoving({ tool: t, opener })}
        />
      </li>
    );
  };
  const group = (title: string, rows: React.ReactNode[]) =>
    rows.length > 0 && (
      <section className="plugin-group">
        <h2>
          {title} <span className="nav-count">{rows.length}</span>
        </h2>
        <ul>{rows}</ul>
      </section>
    );
  const pickedSkill = view.page === 'skill' ? view.name : null;
  const pickedTool = view.page === 'tool' ? view.id : null;
  // The page open, as listed: one gone (removed here, or a skill's folder by hand) goes back to the list.
  const skillPage = skills?.find((s) => s.name === pickedSkill);
  const toolPage = tools?.find((t) => t.id === pickedTool);
  useEffect(() => {
    const goneSkill = pickedSkill && skills && !skills.some((s) => s.name === pickedSkill);
    const goneTool = pickedTool && tools && !tools.some((t) => t.id === pickedTool);
    if (goneSkill || goneTool) setView({ page: 'list' });
  }, [pickedSkill, skills, pickedTool, tools]);

  const dot = (tone: 'attention' | 'danger', label: string) => (
    <i className="plugins-dot" data-tone={tone} role="img" aria-label={label} />
  );
  const title = { all: 'Tools and skills', tools: 'Tools', skills: 'Skills' }[kind];

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
            aria-current={kind === 'tools' ? 'page' : undefined}
            onClick={() => pick('tools')}
          >
            <Plug aria-hidden="true" />
            Tools
          </button>
          <button
            type="button"
            className="navbutton"
            aria-current={kind === 'skills' ? 'page' : undefined}
            onClick={() => pick('skills')}
          >
            <BookOpen aria-hidden="true" />
            Skills
          </button>
        </div>
        <div className="chatlist">
          <section>
            <h2>Installed</h2>
            {nothing && <p className="sidebar-empty">Nothing yet</p>}
            {shownTools.map((t) => (
              <div
                key={`t-${t.id}`}
                className="chatrow"
                data-current={t.id === pickedTool || undefined}
                data-disabled={!t.enabled || undefined}
              >
                <button
                  type="button"
                  className="chat-link"
                  aria-current={t.id === pickedTool ? 'page' : undefined}
                  onClick={() => openTool(t.id)}
                >
                  <Plug aria-hidden="true" className="plugins-nav-icon" />
                  <span>{t.name}</span>
                  {t.enabled && t.state === 'key_refused' && dot('attention', 'Key refused')}
                  {t.enabled && t.state === 'sign_in' && dot('attention', 'Sign in again')}
                  {t.enabled && t.state === 'not_reachable' && dot('danger', 'Not reachable')}
                </button>
              </div>
            ))}
            {shownSkills.map((s) => (
              <div
                key={`s-${s.name}`}
                className="chatrow"
                data-current={s.name === pickedSkill || undefined}
                data-disabled={!s.enabled || undefined}
              >
                <button
                  type="button"
                  className="chat-link"
                  aria-current={s.name === pickedSkill ? 'page' : undefined}
                  onClick={() => openSkill(s.name)}
                >
                  <BookOpen aria-hidden="true" className="plugins-nav-icon" />
                  <span>{s.name}</span>
                  {s.problem && dot('danger', 'Can’t be read')}
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
          {skillPage && (
            <SkillPage
              key={skillPage.name}
              skill={skillPage}
              onBack={() => setView({ page: 'list' })}
              onFailure={setFailure}
              onEnable={enableSkill}
              onRemove={(skill, opener) => setRemoving({ skill, opener })}
            />
          )}
          {toolPage && (
            <ToolPage
              key={toolPage.id}
              tool={toolPage}
              onBack={() => setView({ page: 'list' })}
              onFailure={setFailure}
              onEnable={enableTool}
              onChangeKey={changeKey}
              onRemove={(tool, opener) => setRemoving({ tool, opener })}
              onReference={onReference}
            />
          )}
          {view.page === 'new' && (
            <NewSkill
              onBack={() => setView({ page: 'list' })}
              onSave={(skill) => api.newSkill(skill).then((s) => loadSkills().then(() => openSkill(s.name)))}
              onReference={onReference}
            />
          )}
          {view.page === 'add-tool' && (
            <AddTool
              onBack={() => setView({ page: 'list' })}
              onAdded={(id) => loadTools().then(() => openTool(id))}
              onReference={onReference}
            />
          )}
          {view.page === 'list' && (
            <>
              <h1>{title}</h1>
              <div className="plugins-bar">
                {!nothing && (
                  <>
                    <label className="plugins-search">
                      <Search aria-hidden="true" />
                      <input
                        ref={search}
                        type="text"
                        placeholder="Search"
                        aria-label="Search tools and skills"
                        value={query}
                        onChange={(e) => setQuery(e.target.value)}
                      />
                    </label>
                    <fieldset className="plugins-kind" aria-label="Show">
                      {(['all', 'tools', 'skills'] as const).map((k) => (
                        <button key={k} type="button" aria-pressed={kind === k} onClick={() => pick(k)}>
                          {{ all: 'All', tools: 'Tools', skills: 'Skills' }[k]}
                        </button>
                      ))}
                    </fieldset>
                  </>
                )}
                <AddMenu
                  onTool={() => setView({ page: 'add-tool' })}
                  onFolder={(files) => added(api.addSkillFolder(files))}
                  onZip={(file) => added(api.addSkillZip(file))}
                  onFile={(file) => added(api.addSkillFile(file))}
                  onNew={() => setView({ page: 'new' })}
                />
              </div>
              {nothing ? (
                <div className="plugins-empty">
                  <h2>Nothing installed yet</h2>
                  <p className="muted">
                    A tool lets your agents use an app on the web, such as an issue tracker or a calendar.
                  </p>
                  <p className="muted">
                    A skill is a folder of instructions an agent reads when a task needs them.
                  </p>
                </div>
              ) : (
                <>
                  {group('Tools', shownTools.filter((t) => t.enabled).map(toolRow))}
                  {group('Skills', shownSkills.filter((s) => s.enabled).map(skillRow))}
                  {group('Disabled', [
                    ...shownTools.filter((t) => !t.enabled).map(toolRow),
                    ...shownSkills.filter((s) => !s.enabled).map(skillRow),
                  ])}
                </>
              )}
            </>
          )}
        </div>
      </main>
      {removing && (
        <Confirm
          question={`Remove ${'skill' in removing ? removing.skill.name : removing.tool.name}?`}
          body={
            'skill' in removing
              ? `Its folder is deleted, with all its files: ${removing.skill.path}`
              : removing.tool.signed_in
                ? 'Its sign-in is deleted too.'
                : 'Its key is deleted too.'
          }
          verb="Remove"
          danger
          opener={removing.opener}
          onCancel={() => setRemoving(null)}
          onConfirm={() =>
            ('skill' in removing
              ? api.removeSkill(removing.skill.name)
              : api.removeTool(removing.tool.id)
            ).then(() => {
              setRemoving(null);
              setView({ page: 'list' });
            })
          }
          onReference={onReference}
        />
      )}
      {changingKey && (
        <ChangeKey tool={changingKey.tool} opener={changingKey.opener} onClose={() => setChangingKey(null)} />
      )}
    </>
  );
}

interface AddProps {
  onTool: () => void;
  onFolder: (files: File[]) => void;
  onZip: (file: File) => void;
  onFile: (file: File) => void;
  onNew: () => void;
}

/** Add: a tool on the web; a skill from a folder, from a zip, from one Markdown file, or written here. */
function AddMenu({ onTool, onFolder, onZip, onFile, onNew }: AddProps) {
  const folder = useRef<HTMLInputElement>(null);
  const zip = useRef<HTMLInputElement>(null);
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
        <DropdownMenuContent align="end" sideOffset={6} className="plugins-add-menu">
          <DropdownMenuLabel>Tool</DropdownMenuLabel>
          <DropdownMenuItem onSelect={onTool}>On the web</DropdownMenuItem>
          <DropdownMenuSeparator />
          <DropdownMenuLabel>Skill</DropdownMenuLabel>
          <DropdownMenuItem onSelect={() => folder.current?.click()}>From a folder</DropdownMenuItem>
          <DropdownMenuItem onSelect={() => zip.current?.click()}>From a zip</DropdownMenuItem>
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
        ref={zip}
        type="file"
        accept=".zip,.skill"
        hidden
        onChange={(e) => picked(e.currentTarget, ([f]) => onZip(f))}
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
