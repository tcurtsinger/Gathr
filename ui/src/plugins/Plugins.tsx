import { BookOpen, Plug, Plus, Search } from 'lucide-react';
import { createContext, useCallback, useEffect, useRef, useState } from 'react';
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
import { useCard } from '../components/Corner';
import { IconButton } from '../components/IconButton';
import { listen } from '../live';
import { Screen } from '../Screen';
import { AddPcTool } from './AddPcTool';
import { AddTool } from './AddTool';
import { ChangeKey } from './ChangeKey';
import { NewSkill } from './NewSkill';
import { SkillMenu } from './SkillMenu';
import { SkillPage } from './SkillPage';
import { ToolMenu } from './ToolMenu';
import { ToolPage } from './ToolPage';
import { host, stateOf, where } from './tool';
import './plugins.css';

/** What a tool's page goes straight on to: its key, its sign-in, or Change for a tool on this PC. */
export type ToolNext = 'key' | 'sign-in' | 'change';
/** A skill's page, or a tool's, and then perhaps on to its key, sign-in or Change. */
export type PluginAt = { skill: string } | { tool: string; next?: ToolNext };
/** Opens Plugins at a page: a line in a chat for a tool or skill an agent added calls it. */
export const OpenPlugin = createContext<((at: PluginAt) => void) | undefined>(undefined);

/** The list, a skill's or a tool's page, New skill, Add a tool on the web or on this PC, or Change one on this PC. */
type View =
  | { page: 'list' }
  | { page: 'skill'; name: string }
  | { page: 'tool'; id: string; next?: ToolNext }
  | { page: 'new' }
  | { page: 'add-tool' }
  | { page: 'add-pc' }
  | { page: 'change-pc'; id: string; name: string; text: string };
/** The list's choice: both kinds, or one; the sidebar's Tools and Skills set it too. */
type Kind = 'all' | 'tools' | 'skills';

type Removing = { skill: Skill; opener: HTMLElement | null } | { tool: WebTool; opener: HTMLElement | null };

/**
 * The Plugins screen, in place of the chat list and the chat: its sidebar names every tool and skill installed, the
 * main panel lists them in groups, with one search for both.
 */
export function Plugins({ at }: { at?: PluginAt }) {
  const [skills, setSkills] = useState<Skill[] | null>(null);
  const [tools, setTools] = useState<WebTool[] | null>(null);
  const [view, setView] = useState<View>({ page: 'list' });
  useEffect(() => {
    if (at)
      setView(
        'skill' in at ? { page: 'skill', name: at.skill } : { page: 'tool', id: at.tool, next: at.next },
      );
  }, [at]);
  const [kind, setKind] = useState<Kind>('all');
  const [query, setQuery] = useState('');
  const [failure, setFailure] = useState<Failure | null>(null);
  useCard(failure);
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
    <li key={`s-${s.name}`} className="screen-row" data-disabled={!s.enabled || undefined}>
      <button type="button" className="screen-open" onClick={() => openSkill(s.name)}>
        <span className="plugin-tile" aria-hidden="true">
          <BookOpen />
        </span>
        <span className="screen-words">
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
      <li key={`t-${t.id}`} className="screen-row" data-disabled={!t.enabled || undefined}>
        <button type="button" className="screen-open" onClick={() => openTool(t.id)}>
          <span className="plugin-tile" aria-hidden="true">
            <Plug />
          </span>
          <span className="screen-words">
            <strong>{t.name}</strong>
            <span>
              {where(t)} · <span data-tone={state.tone}>{state.words}</span>
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
      <section className="screen-group">
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
  // A tool that wants something shows a dot in its tone.
  const flag = (t: WebTool) => {
    const state = stateOf(t);
    return state.tone && dot(state.tone, state.words);
  };
  const title = { all: 'Tools and skills', tools: 'Tools', skills: 'Skills' }[kind];

  return (
    <>
      <Screen
        name="Plugins"
        action={
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
        }
        nav={
          <>
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
                      {flag(t)}
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
          </>
        }
      >
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
            next={view.page === 'tool' ? view.next : undefined}
            onBack={() => setView({ page: 'list' })}
            onFailure={setFailure}
            onEnable={enableTool}
            onChangeKey={changeKey}
            onRemove={(tool, opener) => setRemoving({ tool, opener })}
            onChangeRuns={(tool, text) => setView({ page: 'change-pc', id: tool.id, name: tool.name, text })}
          />
        )}
        {view.page === 'new' && (
          <NewSkill
            onBack={() => setView({ page: 'list' })}
            onSave={(skill) => api.newSkill(skill).then((s) => loadSkills().then(() => openSkill(s.name)))}
          />
        )}
        {view.page === 'add-tool' && (
          <AddTool
            onBack={() => setView({ page: 'list' })}
            onAdded={(id) => loadTools().then(() => openTool(id))}
          />
        )}
        {view.page === 'add-pc' && (
          <AddPcTool
            onBack={() => setView({ page: 'list' })}
            onDone={(id) => loadTools().then(() => openTool(id))}
          />
        )}
        {view.page === 'change-pc' && (
          <AddPcTool
            key={view.id}
            changing={view}
            onBack={() => openTool(view.id)}
            onDone={(id) => loadTools().then(() => openTool(id))}
          />
        )}
        {view.page === 'list' && (
          <>
            <h1>{title}</h1>
            <div className="screen-bar">
              {!nothing && (
                <>
                  <label className="search-box plugins-search">
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
                onPc={() => setView({ page: 'add-pc' })}
                onFolder={(files) => added(api.addSkillFolder(files))}
                onZip={(file) => added(api.addSkillZip(file))}
                onFile={(file) => added(api.addSkillFile(file))}
                onNew={() => setView({ page: 'new' })}
              />
            </div>
            {nothing ? (
              <div className="screen-empty">
                <h2>Nothing installed yet</h2>
                <p className="muted">A tool lets agents use an app, like an issue tracker.</p>
                <p className="muted">A skill is instructions an agent reads when a task needs them.</p>
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
      </Screen>
      {removing && (
        <Confirm
          question={`Remove ${'skill' in removing ? removing.skill.name : removing.tool.name}?`}
          body={
            'skill' in removing
              ? `Deletes its folder and files: ${removing.skill.path}`
              : removing.tool.kind === 'pc'
                ? 'Gathr stops and forgets it. The program stays on this PC.'
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
  onPc: () => void;
  onFolder: (files: File[]) => void;
  onZip: (file: File) => void;
  onFile: (file: File) => void;
  onNew: () => void;
}

/** Add: a tool on the web or on this PC; a skill from a folder, from a zip, from one Markdown file, or written here. */
function AddMenu({ onTool, onPc, onFolder, onZip, onFile, onNew }: AddProps) {
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
          <Button type="button" variant="outline" size="sm" className="screen-add">
            <Plus aria-hidden="true" />
            Add
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" sideOffset={6} className="plugins-add-menu">
          <DropdownMenuLabel>Tool</DropdownMenuLabel>
          <DropdownMenuItem onSelect={onTool}>Web</DropdownMenuItem>
          <DropdownMenuItem onSelect={onPc}>This PC</DropdownMenuItem>
          <DropdownMenuSeparator />
          <DropdownMenuLabel>Skill</DropdownMenuLabel>
          <DropdownMenuItem onSelect={() => folder.current?.click()}>Folder</DropdownMenuItem>
          <DropdownMenuItem onSelect={() => zip.current?.click()}>Zip</DropdownMenuItem>
          <DropdownMenuItem onSelect={() => file.current?.click()}>File</DropdownMenuItem>
          <DropdownMenuItem onSelect={onNew}>New</DropdownMenuItem>
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
