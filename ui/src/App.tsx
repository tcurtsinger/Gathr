import { useCallback, useEffect, useRef, useState } from 'react';
import { type Agent, api, type Chat, type Failure, type Project } from './api';
import { ChatView } from './chat/ChatView';
import type { SendFailure } from './chat/messages';
import { Notice } from './components/Notice';
import { listen } from './live';
import { NewProject } from './NewProject';
import { report, timeToPaint } from './report';
import { Sidebar } from './Sidebar';
import { type Page, SettingsPanel, type SettingsTarget } from './settings/SettingsPanel';
import { TasksFlyout } from './tasks/TasksFlyout';
import { TasksTab } from './tasks/TasksTab';
import { usePersonTasks } from './tasks/usePersonTasks';
import { Workspace, WorkspaceOpener } from './tasks/Workspace';
import './app.css';

export function App() {
  const [chats, setChats] = useState<Chat[]>([]);
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
  const person = usePersonTasks();

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
    api.projects().then(setProjects, setFailure);
  }, []);

  // A new chat is a draft until its first message: then the service makes it, in its project.
  const newChat = useCallback((project?: string) => {
    setFresh({ project });
    setSelected(null);
  }, []);
  const keep = (chat: string, f: SendFailure) =>
    setUnsent((u) => ({ ...u, [chat]: [...(u[chat] ?? []), f] }));
  const current = useRef(fresh);
  current.current = fresh;
  const start = (text: string, agent?: string) => {
    const failed = (e: Failure) => ({ id: crypto.randomUUID(), text, agent, failure: e });
    const draft = fresh;
    api.createChat(draft?.project).then(
      (made) => {
        setChats((list) => [made, ...list]);
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
    chats.find((c) => c.id === selected) ?? (fresh && { id: '', title: 'New chat', project: fresh.project });
  const openSettings = (page: Page, turn?: string, opener = document.activeElement as HTMLElement) =>
    setSettings({ page, chat: chat?.id, title: chat?.title, turn, opener });
  const choose = (id: string) => {
    if (id !== selected) report({ name: 'gathr.ui.chat_switch', attrs: { from: selected, to: id } });
    setSelected(id);
    setFresh(null);
  };
  const chatTasks = person.tasks.filter((t) => t.chat === chat?.id);
  const paneOpen = !!chat?.id && !!person.open[chat.id];
  const openReference = (reference: string) =>
    setSettings((s) => ({
      page: 'diagnostics',
      chat: chat?.id,
      title: chat?.title,
      reference,
      opener: s?.opener ?? (document.activeElement as HTMLElement),
    }));

  return (
    <div className="app">
      <Sidebar
        chats={chats}
        projects={projects}
        selected={selected}
        onSelect={choose}
        onNew={newChat}
        onSettings={(opener) => openSettings('engines', undefined, opener)}
        actions={
          <TasksFlyout
            tasks={person.tasks}
            onOpen={(t) => {
              choose(t.chat);
              person.select(t.chat, t.id);
              person.setOpen(t.chat, true);
            }}
          />
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
          headerEnd={
            chat.id &&
            !paneOpen && (
              <WorkspaceOpener waiting={chatTasks.length > 0} onOpen={() => person.setOpen(chat.id, true)} />
            )
          }
        />
      ) : (
        <main className="chat">
          {failure ? (
            <div className="reading">
              <Notice
                danger
                title={failure.error}
                reference={failure.ref ? { ref: failure.ref, open: openReference } : undefined}
              />
            </div>
          ) : (
            <p className="empty">No chats yet</p>
          )}
        </main>
      )}
      {chat?.id && paneOpen && (
        <Workspace waiting={chatTasks.length > 0} onClose={() => person.setOpen(chat.id, false)}>
          <TasksTab
            key={chat.id}
            tasks={chatTasks}
            selected={person.selected[chat.id] ?? null}
            onSelect={(id) => person.select(chat.id, id)}
            drafts={person.drafts}
            onDraft={person.draft}
            onOpenChat={choose}
            onSend={person.send}
            onReference={openReference}
          />
        </Workspace>
      )}
      <NewProject
        open={newProject}
        agents={agents}
        projects={projects}
        onClose={() => setNewProject(false)}
        onCreated={(project) => {
          setProjects((list) => [...list, project]);
          setNewProject(false);
          if (fresh) setFresh({ project: project.id });
        }}
        onReference={openReference}
      />
      <SettingsPanel
        target={settings}
        onPage={(page) => setSettings((s) => s && { ...s, page, turn: undefined, reference: undefined })}
        onReference={openReference}
        onClose={() => setSettings(null)}
      />
    </div>
  );
}
