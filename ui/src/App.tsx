import { useCallback, useEffect, useRef, useState } from 'react';
import { Button } from '@/components/ui/button';
import { type Agent, api, type Chat, type Failure, type Project } from './api';
import { ChatView } from './chat/ChatView';
import type { SendFailure } from './chat/messages';
import { usePauses } from './chat/pauses';
import { Confirm } from './components/Confirm';
import { FailureNotice } from './components/Notice';
import { listen } from './live';
import { NewProject } from './NewProject';
import { report, timeToPaint } from './report';
import { Sidebar } from './Sidebar';
import { SearchFlyout } from './search/SearchFlyout';
import { type Page, SettingsPanel, type SettingsTarget } from './settings/SettingsPanel';
import { TasksFlyout } from './tasks/TasksFlyout';
import { TasksTab } from './tasks/TasksTab';
import { usePersonTasks } from './tasks/usePersonTasks';
import { type FileTab, fileId, Workspace, WorkspaceOpener } from './tasks/Workspace';
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
  const [agents, setAgents] = useState<Agent[]>([]);
  const [projects, setProjects] = useState<Project[]>([]);
  const [newProject, setNewProject] = useState(false);
  const [fresh, setFresh] = useState<{ project?: string } | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [failure, setFailure] = useState<Failure | null>(null);
  const [settings, setSettings] = useState<SettingsTarget | null>(null);
  const drafts = useRef(new Map<string, string>());
  const [unsent, setUnsent] = useState<Record<string, SendFailure[]>>({});
  const [, redraw] = useState(0);
  const person = usePersonTasks(setFailure);
  const pauses = usePauses(setFailure);
  // Each chat's workspace: the files opened from its questions, and the tab shown.
  const [tabs, setTabs] = useState<Record<string, { files: FileTab[]; selected: string }>>({});
  // Each chat's team column, open until the person hides it (the owner's split chat design).
  const [splits, setSplits] = useState<Record<string, boolean>>({});
  // Each chat's plan bar, folded until the person opens it.
  const [plans, setPlans] = useState<Record<string, boolean>>({});
  // A search hit's message (none for a title: the latest), until its chat shows it.
  const [found, setFound] = useState<{ chat: string; message?: string; at: number } | null>(null);

  useEffect(() => {
    const started = performance.now();
    api.chats().then((list) => {
      setChats(list);
      setSelected((s) => s ?? list[0]?.id ?? null);
      timeToPaint('gathr.ui.start', started, { chats: list.length });
    }, setFailure);
  }, []);

  useEffect(() => {
    const load = () => api.agents().then(setAgents, setFailure);
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
    () => Promise.all([api.chats().then(setChats), api.archivedChats().then(setArchived)]),
    [],
  );
  // The open chat, deleted here or in another window, gives way to its neighbour in the sidebar, else New.
  const lists = useRef({ chats, archived, selected });
  lists.current = { chats, archived, selected };
  const leave = useCallback((id: string) => {
    const { chats, archived, selected } = lists.current;
    if (id !== selected) return;
    const gone = [...chats, ...archived].find((c) => c.id === id);
    const next = gone && neighbour(gone, gone.archived ? archived : chats);
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
  const start = (text: string, agent?: string) => {
    const failed = (e: Failure) => ({ id: crypto.randomUUID(), text, agent, failure: e });
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
        api.send(made.id, text, agent).then(
          () => api.chats().then(setChats, setFailure),
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
  const chatTasks = person.tasks.filter((t) => t.chat === chat?.id);
  const paneOpen = !!chat?.id && !!person.open[chat.id];
  const tabsOf = (c: string) => tabs[c] ?? { files: [], selected: 'tasks' };
  const setTab = (c: string, change: (t: { files: FileTab[]; selected: string }) => Partial<typeof t>) =>
    setTabs((all) => ({ ...all, [c]: { ...tabsOf(c), ...change(tabsOf(c)) } }));
  const name = (agent: string | null) => agents.find((a) => a.id === agent)?.name ?? 'An agent';
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

  return (
    <div className="app">
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
        onSettings={(opener) => openSettings('engines', undefined, opener)}
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
                  person.select(t.chat, t.number);
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
          onEditAgent={(agent) =>
            setSettings({ page: 'agents', agent, opener: document.activeElement as HTMLElement })
          }
          draft={drafts.current.get(chat.id) ?? ''}
          onDraft={(text) => {
            drafts.current.set(chat.id, text);
            redraw((n) => n + 1);
          }}
          unsent={unsent[chat.id] ?? []}
          onUnsent={(update) => setUnsent((u) => ({ ...u, [chat.id]: update(u[chat.id] ?? []) }))}
          openSettings={openSettings}
          onNamed={() => api.chats().then(setChats, setFailure)}
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
          split={splits[chat.id] ?? true}
          onSplit={(open) => setSplits((all) => ({ ...all, [chat.id]: open }))}
          plan={!!plans[chat.id]}
          onPlan={(open) => setPlans((all) => ({ ...all, [chat.id]: open }))}
          found={found?.chat === chat.id ? found : undefined}
          onFound={() => setFound(null)}
          notice={notice}
          headerEnd={
            chat.id &&
            !paneOpen && (
              <WorkspaceOpener waiting={chatTasks.length > 0} onOpen={() => person.setOpen(chat.id, true)} />
            )
          }
        />
      ) : (
        <main className="chat">{notice || <p className="empty">No chats yet</p>}</main>
      )}
      {chat?.id && paneOpen && (
        <Workspace
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
          onClose={() => person.setOpen(chat.id, false)}
          tasks={
            <TasksTab
              key={chat.id}
              tasks={chatTasks}
              selected={person.selected[chat.id] ?? null}
              onSelect={(n) => person.select(chat.id, n)}
              drafts={person.drafts}
              onDraft={person.draft}
              name={name}
              onOpenFile={(task, index) => {
                const file = { task: task.number, index, name: task.files[index].name };
                setTab(chat.id, (t) => ({
                  files: t.files.some((f) => fileId(f) === fileId(file)) ? t.files : [...t.files, file],
                  selected: fileId(file),
                }));
              }}
              onSend={person.send}
              onReference={openReference}
              stale={person.stale.filter((t) => t.chat === chat.id)}
              onSettle={(t, action) => person.settle(t.number, action)}
            />
          }
        />
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
      <SettingsPanel
        target={settings}
        onPage={(page) => setSettings((s) => s && { ...s, page, turn: undefined, reference: undefined })}
        onReference={openReference}
        onClose={() => setSettings(null)}
      />
    </div>
  );
}

/** The chat after it in its sidebar group (its project, plain chats, or Archived), else the one before. */
function neighbour(chat: Chat, list: Chat[]): Chat | undefined {
  const group = chat.archived ? list : list.filter((c) => (c.project ?? '') === (chat.project ?? ''));
  const at = group.findIndex((c) => c.id === chat.id);
  return group[at + 1] ?? group[at - 1];
}
