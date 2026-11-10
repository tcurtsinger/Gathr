import { keep, kept } from './kept';
import { report } from './report';

export type Theme = 'system' | 'light' | 'dark';

const key = 'gathr.theme';
const systemLight = matchMedia('(prefers-color-scheme: light)');

export function savedTheme(): Theme {
  const theme = kept(key);
  return theme === 'light' || theme === 'dark' ? theme : 'system';
}

function apply() {
  const theme = savedTheme();
  document.documentElement.classList.toggle(
    'light',
    theme === 'light' || (theme === 'system' && systemLight.matches),
  );
}

/** The desktop window's IPC, allowed only `set_theme` (crates/gathr-app/capabilities); none in a browser. */
type Tauri = { invoke(cmd: string, args: object): Promise<unknown> };

/** The window's title bar takes Theme too; with none it follows the system, as the page does. */
function titleBar(theme: Theme) {
  const tauri = (window as { __TAURI_INTERNALS__?: Tauri }).__TAURI_INTERNALS__;
  tauri
    ?.invoke('plugin:window|set_theme', { value: theme === 'system' ? null : theme })
    .catch((e) =>
      report({ name: 'gathr.ui.window.theme', error: 'set_theme', attrs: { message: String(e) } }),
    );
}

/** Applies the kept theme, then follows the system's setting while that is System. */
export function followTheme() {
  apply();
  titleBar(savedTheme());
  systemLight.addEventListener('change', apply);
}

export function setTheme(theme: Theme) {
  keep(key, theme === 'system' ? undefined : theme);
  apply();
  titleBar(theme);
}
