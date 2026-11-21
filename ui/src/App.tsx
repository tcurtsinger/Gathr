import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Agents } from './agents/Agents';
import {
  type Agent,
  api,
  type Chat,
  type Failure,
  type Named,
  type PickedName,
  type Project,
  type Settings,
} from './api';
import { Confirm } from './components/Confirm';
import { Corner, useCard } from './components/Corner';
import { KnownFaces } from './components/Face';
import type { Picked } from './components/mention';
import { DeleteConfirm } from './DeleteConfirm';
import { Designs } from './designs/Designs';
import { kept, keep as remember } from './kept';
import { listen } from './live';
import { NewProject } from './NewProject';
import { usePauses } from './pauses';
import { Plugins } from './plugins/Plugins';
import { type Opened, Rail } from './Rail';
import { report, timeToPaint } from './report';
import type { Found } from './room/Room';
import { RoomView, type Unsent } from './room/RoomView';
import { openDesign } from './room/Sidebar';
import { Sidebar } from './Sidebar';
import { SearchFlyout } from './search/SearchFlyout';
import { type Page, SettingsPanel, type SettingsTarget } from './settings/SettingsPanel';
import { NavButtons, TopLine } from './TopLine';
import { readyVersion, useAppUpdate } from './update';
import './app.css';

export function App() {
  const [chats, setChats] = useState<Chat[]>([]);
  const [archived, setArchived] = useState<Chat[]>([]);
  // Archive and Delete ask first; the question stays until the service has done it.
  const [asking, setAsking] = useState<{
    verb: 'Archive' | 'Delete';
    chat: Chat;
    opener: HTMLElement | null;
  } | null>(null);
  // Restore asks nothing.
  const [restoreFailure, setRestoreFailure] = useState<Failure | null>(null);
  // A project's Delete, from its More (0069).
  const [deleting, setDeleting] = useState<{ project: Project; opener: HTMLElement | null } | null>(null);
  const [agents, setAgents] = useState<Agent[]>([]);
  const [removed, setRemoved] = useState<Named[]>([]);
  // Every agent's face by id, removed ones too, so a place that knows only an agent's id draws its face.
  const faces = useMemo(() => new Map([...agents, ...removed].map((a) => [a.id, a])), [agents, removed]);
  const [projects, setProjects] = useState<Project[]>([]);
  const [newProject, setNewProject] = useState(false);
  const [fresh, setFresh] = useState<{ project?: string } | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  // What a project's Delete couldn't remove yet.
  const [left, setLeft] = useState<Failure | null>(null);
  const [settings, setSettings] = useState<SettingsTarget | null>(null);
  // Agents, Plugins or Designs, in place of the chat list and the chat until Home or a chat is opened.
  const [opened, setOpened] = useState<Opened>('home');
  // Each chat's draft and the names picked in it, kept while another chat is open.
  const drafts = useRef(new Map<string, { text: string; picks: Picked[]; files?: File[] }>());
  const [unsent, setUnsent] = useState<Record<string, Unsent[]>>({});
  const [, redraw] = useState(0);
  const pauses = usePauses();
  const [updateFailure, setUpdateFailure] = useState<Failure | null>(null);
  const appUpdate = useAppUpdate(setUpdateFailure);
  useCard(restoreFailure);
  useCard(left);
  useCard(updateFailure);
  // Whether each chat's sidebar is open, as the person left it there.
  const [sidebars, setSidebars] = useState<Record<string, boolean>>({});
  // The message a search hit opened its chat at.
  const [found, setFound] = useState<Found>();
  // The chat list's column, closed or open as the person left it on this computer.
  const [nav, setNav] = useState(() => kept('gathr.nav') !== 'closed');
  const showNav = (open: boolean) => {
    setNav(open);
    remember('gathr.nav', open ? undefined : 'closed');
  };

  // A chat's state told while a list loads may be newer than that list: it is put back on top of it.
  const loading = useRef<Map<string, Pick<Chat, 'working' | 'needs' | 'mark'>>[]>([]);
  const loadChats = useCallback(() => {
    const told = new Map<string, Pick<Chat, 'working' | 'needs' | 'mark'>>();
    loading.current.push(told);
    const done = () => {
      loading.current = loading.current.filter((t) => t !== told);
    };
    return api
      .chats()
      .then((list) => list.map((c) => ({ ...c, ...told.get(c.id) })))
      .finally(done);
  }, []);
  useEffect(
    () =>
      listen((event) => {
        if (event.type !== 'chat_state') return;
        const state = { working: event.working, needs: event.needs, mark: event.mark ?? null };
        for (const told of loading.current) told.set(event.chat, state);
        setChats((list) => list.map((c) => (c.id === event.chat ? { ...c, ...state } : c)));
      }),
    [],
  );

  useEffect(() => {
    const started = performance.now();
    loadChats().then((list) => {
      setChats(list);
      setSelected((s) => s ?? list[0]?.id ?? null);
      // With no chat to open, the window opens on New.
      if (list.length === 0) setFresh((f) => f ?? {});
      timeToPaint('gathr.ui.start', started, { chats: list.length });
    }, quiet);
  }, [loadChats]);

  useEffect(() => {
    const load = () =>
      api.agents().then((a) => {
        setAgents(a.agents);
        setRemoved(a.removed);
      }, quiet);
    load();
    return listen((event) => {
      if (event.type === 'agents' || event.type === 'lagged') load();
    });
  }, []);

  // The person's name and assistant, for a new chat's greeting: read again as Settings closes, as Agents changes the
  // assistant, and when agents change (deleting the assistant clears it).
  const [you, setYou] = useState<Pick<Settings, 'person_name' | 'assistant'>>({});
  const loadYou = useCallback(() => api.settings().then(setYou, quiet), []);
  useEffect(() => {
    loadYou();
    return listen((event) => {
      if (event.type === 'agents' || event.type === 'lagged') loadYou();
    });
  }, [loadYou]);

  useEffect(() => {
    const load = () => api.projects().then(setProjects, quiet);
    load();
    return listen((event) => {
      if (event.type === 'project' || event.type === 'lagged') load();
    });
  }, []);

  const current = useRef(fresh);
  current.current = fresh;
  // Made, archived, restored or deleted here or elsewhere: both lists as the service has them.
  const reloadLists = useCallback(
    () => Promise.all([loadChats().then(setChats), api.archivedChats().then(setArchived)]),
    [loadChats],
  );
  // The open chat, deleted here or in another window, gives way to its neighbour in the sidebar, else New.
  const lists = useRef({ chats, archived, selected });
  lists.current = { chats, archived, selected };
  // Chats deleted, or going with a project deleted here: a project's or an agent's delete deletes several
  // one after another, and the open one never gives way to one of them.
  const going = useRef(new Set<string>());
  const leave = useCallback((id: string) => {
    going.current.add(id);
    const { chats, archived, selected } = lists.current;
    if (id !== selected) return;
    const gone = [...chats, ...archived].find((c) => c.id === id);
    const staying = (list: Chat[]) => list.filter((c) => c.id === id || !going.current.has(c.id));
    const next = gone && neighbour(gone, staying(gone.archived ? archived : chats));
    setSelected(next?.id ?? null);
    if (!next) setFresh({});
  }, []);
  useEffect(() => {
    api.archivedChats().then(setArchived, quiet);
    return listen((event) => {
      if (event.type === 'chat' && event.change === 'deleted') leave(event.chat);
      if (event.type !== 'chat' && event.type !== 'lagged') return;
      // A chat made elsewhere opens only when nothing, not even a draft, is open once the lists are in:
      // the person may have clicked New while they loaded.
      const created = event.type === 'chat' && event.change === 'created';
      reloadLists().then(() => created && !current.current && setSelected((s) => s ?? event.chat), quiet);
    });
  }, [reloadLists, leave]);

  // A new chat is a draft until its first message: then the service makes it, in its project.
  const newChat = useCallback((project?: string) => {
    setFresh({ project });
    setSelected(null);
    setOpened('home');
  }, []);
  const keep = (chat: string, f: Unsent) => setUnsent((u) => ({ ...u, [chat]: [...(u[chat] ?? []), f] }));
  const start = (text: string, agent?: string, picks?: PickedName[], files?: readonly File[]) => {
    const failed = (e: Failure) => ({ id: crypto.randomUUID(), text, agent, picks, files, failure: e });
    const draft = fresh;
    api.createChat(draft?.project).then(
      (made) => {
        // The live "created" may have reloaded the list with it already.
        setChats((list) => [made, ...list.filter((c) => c.id !== made.id)]);
        // Only while the person is still on this draft: another chat, or a newer draft, stays open.
        if (current.current === draft) {
          setFresh(null);
          setSelected(made.id);
        }
        api.send(made.id, text, agent, picks, undefined, files).then(
          () => loadChats().then(setChats, quiet),
          (e: Failure) => keep(made.id, failed(e)),
        );
      },
      (e: Failure) => keep('', failed(e)),
    );
  };

  const chat =
    [...chats, ...archived].find((c) => c.id === selected) ??
    (fresh && { id: '', title: 'New chat', project: fresh.project });
  const openSettings = (page: Page, turn?: string, opener = document.activeElement as HTMLElement) =>
    setSettings({ page, chat: chat?.id, title: chat?.title, turn, opener });
  const choose = (id: string) => {
    if (id !== selected) report({ name: 'gathr.ui.chat_switch', attrs: { from: selected, to: id } });
    setSelected(id);
    setFresh(null);
    setOpened('home');
  };
  // What the main area showed this run, a chat's id, '' for New, or a screen's name, for Back and Forward.
  // Opening something after going back drops what was ahead, as a browser does; a chat deleted since is skipped.
  const [trail, setTrail] = useState<{ list: string[]; at: number }>({ list: [], at: -1 });
  const shown = opened !== 'home' ? opened : (selected ?? (fresh ? '' : null));
  useEffect(() => {
    if (shown === null) return;
    setTrail((t) =>
      t.list[t.at] === shown ? t : { list: [...t.list.slice(0, t.at + 1), shown], at: t.at + 1 },
    );
  }, [shown]);
  const there = (key: string) =>
    key !== shown && (key === '' || away(key) || [...chats, ...archived].some((c) => c.id === key));
  const back = trail.list.findLastIndex((key, i) => i < trail.at && there(key));
  const ahead = trail.list.findIndex((key, i) => i > trail.at && there(key));
  const go = (at: number) => {
    setTrail((t) => ({ ...t, at }));
    const key = trail.list[at];
    if (away(key)) setOpened(key);
    else if (key) choose(key);
    else newChat();
  };
  // A click on a system notification opens its chat: the desktop app calls this (0058).
  const chooseNow = useRef(choose);
  chooseNow.current = choose;
  useEffect(() => {
    const page = window as { gathrOpenChat?: (id: string) => void };
    page.gathrOpenChat = (id) => {
      const { chats, archived } = lists.current;
      if ([...chats, ...archived].some((c) => c.id === id)) chooseNow.current(id);
      else report({ name: 'gathr.ui.notify', error: 'gone', attrs: { outcome: 'clicked', chat: id } });
    };
    return () => {
      delete page.gathrOpenChat;
    };
  }, []);
  // An archived chat stays open, read only; a deleted one gives way to its neighbour in the sidebar.
  const archive = (c: Chat) => api.archive(c.id).then(reloadLists);
  const restore = (c: Chat) => {
    setRestoreFailure(null);
    api.unarchive(c.id).then(reloadLists).catch(setRestoreFailure);
  };
  const remove = async (c: Chat) => {
    await api.deleteChat(c.id);
    leave(c.id);
    await reloadLists().catch(quiet);
  };
  // Its chats went one by one, each told on the live socket as a chat's Delete is.
  const deleted = (project: string, left: Failure | null) => {
    setDeleting(null);
    setProjects((list) => list.filter((p) => p.id !== project));
    setLeft(left);
    reloadLists().catch(quiet);
  };
  // While its confirm is open, a project's chats are going: none of them is a neighbour to move to.
  const projectChats = (project: string) => [...chats, ...archived].filter((c) => c.project === project);
  const askDelete = (project: Project, opener: HTMLElement | null) => {
    for (const c of projectChats(project.id)) going.current.add(c.id);
    setDeleting({ project, opener });
  };
  const cancelDelete = (project: string) => {
    for (const c of projectChats(project)) going.current.delete(c.id);
    setDeleting(null);
  };
  const name = (agent: string | null) =>
    [...agents, ...removed].find((a) => a.id === agent)?.name ?? 'An agent';
  // A chat this window hasn't loaded yet: load the lists first.
  const openChat = (id: string) => {
    if ([...chats, ...archived].some((c) => c.id === id)) choose(id);
    else
      reloadLists()
        .catch(quiet)
        .finally(() => choose(id));
  };
  const openReference = (reference: string) =>
    setSettings((s) => ({
      page: 'turns',
      chat: chat?.id,
      title: chat?.title,
      reference,
      opener: s?.opener ?? (document.activeElement as HTMLElement),
    }));

  const screen = chat && {
    chat,
    agents,
    removed,
    you,
    draft: drafts.current.get(chat.id)?.text ?? '',
    picks: drafts.current.get(chat.id)?.picks,
    onDraft: (text: string, picks: Picked[] = []) => {
      drafts.current.set(chat.id, { ...drafts.current.get(chat.id), text, picks });
      redraw((n) => n + 1);
    },
    files: drafts.current.get(chat.id)?.files,
    onFiles: (files: File[]) => {
      drafts.current.set(chat.id, { text: '', picks: [], ...drafts.current.get(chat.id), files });
      redraw((n) => n + 1);
    },
    unsent: unsent[chat.id] ?? [],
    onUnsent: (update: (list: Unsent[]) => Unsent[]) =>
      setUnsent((u) => ({ ...u, [chat.id]: update(u[chat.id] ?? []) })),
    onNamed: () => loadChats().then(setChats, quiet),
    projects,
    onProject: (project: string) => setFresh({ project }),
    onStart: start,
    onNewProject: () => setNewProject(true),
    pauses: pauses.pauses,
    onPause: pauses.act,
    onArchive: (opener: HTMLElement | null) => setAsking({ verb: 'Archive', chat, opener }),
    onDelete: (opener: HTMLElement | null) => setAsking({ verb: 'Delete', chat, opener }),
    onRestore: () => restore(chat),
  };

  const app = (
    <div className={nav ? 'app' : 'app nav-closed'}>
      <NavButtons
        open={nav}
        lists={opened === 'home' ? 'chats' : opened}
        onToggle={() => showNav(!nav)}
        onBack={back < 0 ? undefined : () => go(back)}
        onForward={ahead < 0 ? undefined : () => go(ahead)}
      />
      <Rail
        open={opened}
        onOpen={setOpened}
        onSettings={(opener) => openSettings('engines', undefined, opener)}
        update={{
          ready: readyVersion(appUpdate.app),
          updating: appUpdate.app?.state === 'updating',
          asking: appUpdate.asking,
          onUpdate: appUpdate.update,
        }}
      />
      {opened === 'agents' && <Agents onAssistant={loadYou} />}
      {opened === 'plugins' && <Plugins />}
      {opened === 'designs' && (
        <Designs
          onChat={(chat, design) => {
            openDesign(chat, design.path, design.name);
            setSidebars((all) => ({ ...all, [chat]: true }));
            openChat(chat);
          }}
        />
      )}
      {opened === 'home' && (
        <Sidebar
          chats={chats}
          archived={archived}
          projects={projects}
          onArchive={(c, opener) => setAsking({ verb: 'Archive', chat: c, opener })}
          onDelete={(c, opener) => setAsking({ verb: 'Delete', chat: c, opener })}
          onRestore={restore}
          selected={selected}
          onSelect={choose}
          onNew={newChat}
          onDeleteProject={askDelete}
          onProjectSettings={(project, opener) =>
            setSettings({
              page: 'projects',
              project,
              opener: opener ?? (document.activeElement as HTMLElement),
            })
          }
          actions={
            <SearchFlyout
              name={name}
              onShortcut={() => showNav(true)}
              onOpen={(hit) => {
                setFound('message' in hit ? { chat: hit.chat, message: hit.message } : undefined);
                openChat(hit.chat);
              }}
            />
          }
        />
      )}
      {opened !== 'home' ? null : screen ? (
        <RoomView
          key={screen.chat.id || 'new'}
          {...screen}
          found={found?.chat === screen.chat.id ? found : undefined}
          sidebar={sidebars[screen.chat.id]}
          onSidebar={(open) => setSidebars((all) => ({ ...all, [screen.chat.id]: open }))}
        />
      ) : (
        <>
          <TopLine />
          <main className="chat" />
        </>
      )}
      <NewProject
        open={newProject}
        agents={agents}
        projects={projects}
        onClose={() => setNewProject(false)}
        onCreated={(project) => {
          setProjects((list) => [...list.filter((p) => p.id !== project.id), project]);
          setNewProject(false);
          if (fresh) setFresh({ project: project.id });
        }}
      />
      {asking && (
        <Confirm
          key={`confirm-${asking.chat.id}`}
          question={`${asking.verb} ${asking.chat.title}?`}
          verb={asking.verb}
          danger={asking.verb === 'Delete'}
          opener={asking.opener}
          onCancel={() => setAsking(null)}
          onConfirm={() =>
            (asking.verb === 'Archive' ? archive : remove)(asking.chat).then(() => setAsking(null))
          }
        />
      )}
      {deleting && (
        <DeleteConfirm
          key={`delete-${deleting.project.id}`}
          kind="project"
          id={deleting.project.id}
          name={deleting.project.name}
          opener={deleting.opener}
          onCancel={() => cancelDelete(deleting.project.id)}
          onDeleted={(left) => deleted(deleting.project.id, left)}
        />
      )}
      <SettingsPanel
        target={settings}
        onPage={(page) =>
          setSettings((s) => s && { ...s, page, project: undefined, turn: undefined, reference: undefined })
        }
        onClose={() => {
          setSettings(null);
          loadYou();
        }}
      />
      <Corner onReference={openReference} />
    </div>
  );
  return <KnownFaces.Provider value={faces}>{app}</KnownFaces.Provider>;
}

/** What the app only loads or reloads by itself says nothing when it fails: the live socket reloads it. */
const quiet = () => {};

/** The screens' places in Back and Forward are their names: never a chat's id. */
const away = (key: string): key is Exclude<Opened, 'home'> =>
  key === 'agents' || key === 'plugins' || key === 'designs';

/** The chat after it in its sidebar group (its project, plain chats, or Archived), else the one before. */
function neighbour(chat: Chat, list: Chat[]): Chat | undefined {
  const group = chat.archived ? list : list.filter((c) => (c.project ?? '') === (chat.project ?? ''));
  const at = group.findIndex((c) => c.id === chat.id);
  return group[at + 1] ?? group[at - 1];
}
