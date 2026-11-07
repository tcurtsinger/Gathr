import { useCallback, useEffect, useRef, useState } from 'react';
import { api, type Chat, type Failure } from './api';
import { ChatView } from './chat/ChatView';
import type { SendFailure } from './chat/rows';
import { Notice } from './components/Notice';
import { report, timeToPaint } from './report';
import { Sidebar } from './Sidebar';
import { type Page, SettingsPanel, type SettingsTarget } from './settings/SettingsPanel';
import './app.css';

export function App() {
  const [chats, setChats] = useState<Chat[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [failure, setFailure] = useState<Failure | null>(null);
  const [settings, setSettings] = useState<SettingsTarget | null>(null);
  const drafts = useRef(new Map<string, string>());
  const [unsent, setUnsent] = useState<Record<string, SendFailure[]>>({});
  const [, redraw] = useState(0);

  useEffect(() => {
    const started = performance.now();
    api.chats().then((list) => {
      setChats(list);
      setSelected((s) => s ?? list[0]?.id ?? null);
      timeToPaint('gathr.ui.start', started, { chats: list.length });
    }, setFailure);
  }, []);

  const newChat = useCallback(() => {
    api.createChat().then((chat) => {
      setChats((list) => [chat, ...list]);
      setSelected(chat.id);
    }, setFailure);
  }, []);

  const chat = chats.find((c) => c.id === selected);
  const openSettings = (page: Page, turn?: string, opener = document.activeElement as HTMLElement) =>
    setSettings({ page, chat: chat?.id, title: chat?.title, turn, opener });

  return (
    <div className="app">
      <Sidebar
        chats={chats}
        selected={selected}
        onSelect={(id) => {
          if (id !== selected) report({ name: 'gathr.ui.chat_switch', attrs: { from: selected, to: id } });
          setSelected(id);
        }}
        onNew={newChat}
        onSettings={(opener) => openSettings('engines', undefined, opener)}
      />
      {chat ? (
        <ChatView
          key={chat.id}
          chat={chat}
          draft={drafts.current.get(chat.id) ?? ''}
          onDraft={(text) => {
            drafts.current.set(chat.id, text);
            redraw((n) => n + 1);
          }}
          unsent={unsent[chat.id] ?? []}
          onUnsent={(update) => setUnsent((u) => ({ ...u, [chat.id]: update(u[chat.id] ?? []) }))}
          openSettings={openSettings}
          onNamed={() => api.chats().then(setChats, setFailure)}
        />
      ) : (
        <main className="chat">
          {failure ? (
            <div className="reading">
              <Notice
                danger
                title={failure.error}
                reference={failure.ref ? { ref: failure.ref, open: () => {} } : undefined}
              />
            </div>
          ) : (
            <p className="empty">No chats yet</p>
          )}
        </main>
      )}
      <SettingsPanel
        target={settings}
        onPage={(page) => setSettings((s) => s && { ...s, page, turn: undefined })}
        onClose={() => setSettings(null)}
      />
    </div>
  );
}
