import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { App } from './App';
import { preloadCodeBlock } from './components/Markdown';
import { followTheme } from './theme';
import './theme.css';

followTheme();
preloadCodeBlock();
createRoot(document.getElementById('root') as HTMLElement).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
