export type Theme = 'system' | 'light' | 'dark';

// Kept by the window (its webview's storage), so each computer has its own.
const key = 'gathr.theme';
const systemLight = matchMedia('(prefers-color-scheme: light)');

export function savedTheme(): Theme {
  const theme = localStorage.getItem(key);
  return theme === 'light' || theme === 'dark' ? theme : 'system';
}

function apply() {
  const theme = savedTheme();
  document.documentElement.classList.toggle(
    'light',
    theme === 'light' || (theme === 'system' && systemLight.matches),
  );
}

/** Applies the kept theme, then follows the system's setting while that is System. */
export function followTheme() {
  apply();
  systemLight.addEventListener('change', apply);
}

export function setTheme(theme: Theme) {
  if (theme === 'system') localStorage.removeItem(key);
  else localStorage.setItem(key, theme);
  apply();
}
