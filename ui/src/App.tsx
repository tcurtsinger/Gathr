import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { flushSync } from 'react-dom';
import { Button } from '@/components/ui/button';
import {
  type Agent,
  api,
  type Chat,
  type Failure,
  type Named,
  type PickedName,
  type Project,
  type Task,
} from './api';
import { ChatView } from './chat/ChatView';
import type { SendFailure } from './chat/messages';
import { usePauses } from './chat/pauses';
import { Confirm } from './components/Confirm';
import { KnownFaces } from './components/Face';
import type { Picked } from './components/mention';
import { FailureNotice } from './components/Notice';
import { DeleteConfirm } from './DeleteConfirm';
import { listen } from './live';
import { NewProject } from './NewProject';
import { PlanTab } from './plans/PlanTab';
import { Rail } from './Rail';
import { report, timeToPaint } from './report';
import { Sidebar } from './Sidebar';
import { SearchFlyout } from './search/SearchFlyout';
import { type Page, SettingsPanel, type SettingsTarget } from './settings/SettingsPanel';
import { TopLine } from './TopLine';
import { asQuestion } from './tasks/rows';
import { TasksFlyout } from './tasks/TasksFlyout';
import { TasksTab } from './tasks/TasksTab';
import { usePersonTasks } from './tasks/usePersonTasks';
import { type FileTab, fileId, Workspace, WorkspaceToggle } from './tasks/Workspace';
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
  // Restore asks nothing: a failure shows on the archived chat, opened if it wasn't.
  const [restoreFailure, setRestoreFailure] = useState<{ chat: string; failure: Failure } | null>(null);
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
  const [failure, setFailure] = useState<Failure | null>(null);
  const [settings, setSettings] = useState<SettingsTarget | null>(null);
  // Each chat's draft and the names picked in it, kept while another chat is open.
  const drafts = useRef(new Map<string, { text: string; picks: Picked[] }>());
  const [unsent, setUnsent] = useState<Record<string, SendFailure[]>>({});
  const [, redraw] = useState(0);
  const person = usePersonTasks(setFailure);
  const pauses = usePauses(setFailure);
  const appUpdate = useAppUpdate(setFailure);
  // Each chat's workspace: the files opened from its questions, the agent opened from Summary, the plan opened
  // from the timeline, and the tab shown.
  const [tabs, setTabs] = useState<Record<string, Tabs>>({});
  // The task each chat's Tasks tab shows, opened from its list, the chat's Open task or the Tasks flyout; none
  // shows the list.
  const [opened, setOpened] = useState<Record<string, number | undefined>>({});
  // A search hit's message (none for a title: the latest), until its chat shows it.
  const [found, setFound] = useState<{ chat: string; message?: string; at: number } | null>(null);

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
      timeToPaint('gathr.ui.start', started, { chats: list.length });
    }, setFailure);
  }, [loadChats]);

  useEffect(() => {
    const load = () =>
      api.agents().then((a) => {
        setAgents(a.agents);
        setRemoved(a.removed);
      }, setFailure);
    load();
    return listen((event) => {
      if (event.type === 'agents' || event.type === 'lagged') load();
    });
  }, []);

  useEffect(() => {
    const load = () => api.projects().then(setProjects, setFailure);
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
    api.archivedChats().then(setArchived, setFailure);
    return listen((event) => {
      if (event.type === 'chat' && event.change === 'deleted') leave(event.chat);
      if (event.type !== 'chat' && event.type !== 'lagged') return;
      // A chat made elsewhere opens only when nothing, not even a draft, is open once the lists are in:
      // the person may have clicked New while they loaded.
      const created = event.type === 'chat' && event.change === 'created';
      reloadLists().then(
        () => created && !current.current && setSelected((s) => s ?? event.chat),
        setFailure,
      );
    });
  }, [reloadLists, leave]);

  // A new chat is a draft until its first message: then the service makes it, in its project.
  const newChat = useCallback((project?: string) => {
    setFresh({ project });
    setSelected(null);
  }, []);
  const keep = (chat: string, f: SendFailure) =>
    setUnsent((u) => ({ ...u, [chat]: [...(u[chat] ?? []), f] }));
  const start = (text: string, agent?: string, picks?: PickedName[]) => {
    const failed = (e: Failure) => ({ id: crypto.randomUUID(), text, agent, picks, failure: e });
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
        api.send(made.id, text, agent, picks).then(
          () => loadChats().then(setChats, setFailure),
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
    api
      .unarchive(c.id)
      .then(reloadLists)
      .catch((f: Failure) => {
        choose(c.id);
        setRestoreFailure({ chat: c.id, failure: f });
      });
  };
  const remove = async (c: Chat) => {
    await api.deleteChat(c.id);
    leave(c.id);
    await reloadLists().catch(setFailure);
  };
  // Its chats went one by one, each told on the live socket as a chat's Delete is.
  const deleted = (project: string, left: Failure | null) => {
    setDeleting(null);
    setProjects((list) => list.filter((p) => p.id !== project));
    if (left) setFailure(left);
    reloadLists().catch(setFailure);
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
  const chatTasks = person.tasks.filter((t) => t.chat === chat?.id);
  // One draft and one Send per question, in the chat and in the Tasks view alike (0073).
  const answers = { drafts: person.drafts, onDraft: person.draft, onSend: person.send };
  const paneOpen = !!chat?.id && !!person.open[chat.id];
  const tabsOf = (c: string): Tabs => tabs[c] ?? { files: [], selected: 'tasks' };
  const setTab = (c: string, change: (t: Tabs) => Partial<Tabs>) =>
    setTabs((all) => ({ ...all, [c]: { ...tabsOf(c), ...change(tabsOf(c)) } }));
  const name = (agent: string | null) =>
    [...agents, ...removed].find((a) => a.id === agent)?.name ?? 'An agent';
  // A task opened from the chat or the plan's tab: the panel's Tasks, at that task.
  const openTask = (of: string, task: number) => {
    setOpened((all) => ({ ...all, [of]: task }));
    person.setOpen(of, true);
    setTab(of, () => ({ selected: 'tasks' }));
  };
  // The plan's tab at the version shown; the newest when the plan has no such version yet.
  const planTab = (of: string, plan: Tabs['plan'], tasks: Task[]) =>
    plan && {
      version: plan.version,
      waits: plan.waits,
      view: (
        <PlanTab
          key={plan.id}
          chat={of}
          plan={plan.id}
          version={plan.version}
          name={name}
          tasks={tasks}
          onOpenTask={(task) => openTask(of, task)}
          onVersion={(version) => setTab(of, () => ({ plan: { ...plan, version } }))}
          onWaits={(waits) => setTab(of, (t) => ({ plan: t.plan && { ...t.plan, waits } }))}
          onReference={openReference}
        />
      ),
      onClose: () =>
        setTab(of, (t) => ({ plan: undefined, selected: t.selected === 'plan' ? 'tasks' : t.selected })),
    };
  // A chat this window hasn't loaded yet: load the lists first; a failure shows in its place.
  const openChat = (id: string, then: () => void) => {
    const open = () => {
      choose(id);
      then();
    };
    if ([...chats, ...archived].some((c) => c.id === id)) open();
    else reloadLists().catch(setFailure).finally(open);
  };
  const openReference = (reference: string) =>
    setSettings((s) => ({
      page: 'diagnostics',
      chat: chat?.id,
      title: chat?.title,
      reference,
      opener: s?.opener ?? (document.activeElement as HTMLElement),
    }));
  // The latest failure of what the app loads or does itself, over whatever is open, until dismissed.
  const notice = failure && (
    <div className="reading app-failure">
      <FailureNotice
        failure={failure}
        onReference={openReference}
        action={
          <Button type="button" variant="outline" onClick={() => setFailure(null)}>
            Dismiss
          </Button>
        }
      />
    </div>
  );

  const app = (
    <div className="app">
      <Rail
        onSettings={(opener) => openSettings('engines', undefined, opener)}
        update={{
          ready: readyVersion(appUpdate.app),
          updating: appUpdate.app?.state === 'updating',
          asking: appUpdate.asking,
          onUpdate: appUpdate.update,
        }}
      />
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
          <>
            <SearchFlyout
              name={name}
              onReference={openReference}
              onOpen={(hit, at) =>
                openChat(hit.chat, () =>
                  setFound({ chat: hit.chat, message: 'message' in hit ? hit.message : undefined, at }),
                )
              }
            />
            <TasksFlyout
              tasks={person.tasks}
              stale={person.stale}
              name={name}
              onOpen={(t) =>
                openChat(t.chat, () => {
                  setOpened((all) => ({ ...all, [t.chat]: t.number }));
                  person.setOpen(t.chat, true);
                  setTab(t.chat, () => ({ selected: 'tasks' }));
                })
              }
            />
          </>
        }
      />
      {chat ? (
        <ChatView
          key={chat.id || 'new'}
          chat={chat}
          agents={agents}
          removed={removed}
          draft={drafts.current.get(chat.id)?.text ?? ''}
          picks={drafts.current.get(chat.id)?.picks}
          onDraft={(text, picks = []) => {
            drafts.current.set(chat.id, { text, picks });
            redraw((n) => n + 1);
          }}
          unsent={unsent[chat.id] ?? []}
          onUnsent={(update) => setUnsent((u) => ({ ...u, [chat.id]: update(u[chat.id] ?? []) }))}
          openSettings={openSettings}
          onNamed={() => loadChats().then(setChats, setFailure)}
          projects={projects}
          onProject={(project) => setFresh({ project })}
          onStart={start}
          onNewProject={() => setNewProject(true)}
          pauses={pauses.pauses}
          onPause={pauses.act}
          onArchive={(opener) => setAsking({ verb: 'Archive', chat, opener })}
          onDelete={(opener) => setAsking({ verb: 'Delete', chat, opener })}
          onRestore={() => restore(chat)}
          restoreFailure={restoreFailure?.chat === chat.id ? restoreFailure.failure : undefined}
          onReference={openReference}
          onOpenTask={(task) => openTask(chat.id, task)}
          agentOpen={agentOpen(paneOpen ? tabsOf(chat.id) : undefined)}
          onOpenAgent={(agent) => {
            // Drawn at once, so the keyboard can follow to the agent's tab.
            flushSync(() => {
              setTab(chat.id, () => ({ agent, selected: 'agent' }));
              person.setOpen(chat.id, true);
            });
            document.getElementById('workspace-tab-agent')?.focus();
          }}
          onOpenPlan={(id, version) => {
            setTab(chat.id, (t) => ({
              plan: { id, version, waits: t.plan?.id === id && t.plan.waits },
              selected: 'plan',
            }));
            person.setOpen(chat.id, true);
          }}
          found={found?.chat === chat.id ? found : undefined}
          onFound={() => setFound(null)}
          answers={answers}
          notice={notice}
          workspace={(agent, work) =>
            paneOpen && (
              <Workspace
                agent={agent}
                onCloseAgent={() =>
                  setTab(chat.id, (t) => ({
                    agent: undefined,
                    selected: t.selected === 'agent' ? 'tasks' : t.selected,
                  }))
                }
                plan={planTab(chat.id, tabsOf(chat.id).plan, work.tasks)}
                waiting={chatTasks.length > 0}
                files={tabsOf(chat.id).files}
                selected={tabsOf(chat.id).selected}
                onSelect={(id) => setTab(chat.id, () => ({ selected: id }))}
                onCloseFile={(id) =>
                  setTab(chat.id, (t) => ({
                    files: t.files.filter((f) => fileId(f) !== id),
                    selected: t.selected === id ? 'tasks' : t.selected,
                  }))
                }
                tasks={
                  <TasksTab
                    key={chat.id}
                    tasks={work.tasks}
                    members={work.members}
                    opened={opened[chat.id]}
                    onOpen={(n) => setOpened((all) => ({ ...all, [chat.id]: n }))}
                    drafts={person.drafts}
                    onDraft={person.draft}
                    name={name}
                    onOpenFile={(task, index) => {
                      const file = { task: task.number, index, name: task.files?.[index]?.name ?? '' };
                      setTab(chat.id, (t) => ({
                        files: t.files.some((f) => fileId(f) === fileId(file)) ? t.files : [...t.files, file],
                        selected: fileId(file),
                      }));
                    }}
                    onAnswer={(task, draft) => person.send(asQuestion(task, work.tasks), draft)}
                    onReference={openReference}
                    onSettle={(t, action) => person.settle(t.number, action)}
                    project={chat.project}
                  />
                }
              />
            )
          }
          headerEnd={
            chat.id && (
              <WorkspaceToggle
                open={paneOpen}
                waiting={chatTasks.length > 0}
                onToggle={() => person.setOpen(chat.id, !paneOpen)}
              />
            )
          }
        />
      ) : (
        <>
          <TopLine />
          <main className="chat">{notice || <p className="empty">No chats yet</p>}</main>
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
        onReference={openReference}
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
          onReference={openReference}
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
          onReference={openReference}
        />
      )}
      <SettingsPanel
        target={settings}
        onPage={(page) => setSettings((s) => s && { ...s, page, turn: undefined, reference: undefined })}
        onReference={openReference}
        onClose={() => setSettings(null)}
      />
    </div>
  );
  return <KnownFaces.Provider value={faces}>{app}</KnownFaces.Provider>;
}

/** The agent whose tab a chat's workspace has, and whether it is the tab shown. */
const agentOpen = (tabs?: Tabs) =>
  tabs?.agent === undefined ? undefined : { agent: tabs.agent, shown: tabs.selected === 'agent' };

/**
 * A chat's workspace tabs: its opened files, the agent whose panel is open, the plan at the version shown, and
 * the tab shown.
 */
interface Tabs {
  files: FileTab[];
  agent?: string;
  /** `waits`: the plan waits on the person, which puts a dot on its tab. */
  plan?: { id: number; version: number; waits?: boolean };
  selected: string;
}

/** The chat after it in its sidebar group (its project, plain chats, or Archived), else the one before. */
function neighbour(chat: Chat, list: Chat[]): Chat | undefined {
  const group = chat.archived ? list : list.filter((c) => (c.project ?? '') === (chat.project ?? ''));
  const at = group.findIndex((c) => c.id === chat.id);
  return group[at + 1] ?? group[at - 1];
}
