import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { App } from './App';
import { preloadCodeBlock } from './components/Markdown';
import { followTheme } from './theme';
import './theme.css';

followTheme();
preloadCodeBlock();
// A file dropped outside the chat column would make the window open it in place of Gathr.
for (const kind of ['dragover', 'drop']) {
  window.addEventListener(kind, (e) => {
    if ((e as DragEvent).dataTransfer?.types.includes('Files')) e.preventDefault();
  });
}
createRoot(document.getElementById('root') as HTMLElement).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
