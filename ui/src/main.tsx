import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { App } from './App';
import './theme.css';

// The theme follows the system's light or dark setting.
const light = matchMedia('(prefers-color-scheme: light)');
const theme = () => document.documentElement.classList.toggle('light', light.matches);
theme();
light.addEventListener('change', theme);

createRoot(document.getElementById('root') as HTMLElement).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
