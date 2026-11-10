// The window's own preferences, in cookies rather than localStorage: localStorage is kept per origin, so
// per port, and the service may come back on another port; a cookie is kept per host (0004). Each window's
// webview has its own, so each computer does too. A read renews it: browsers cap a cookie's age.
const age = 400 * 24 * 60 * 60;

export function kept(name: string): string | undefined {
  const value = document.cookie
    .split('; ')
    .find((pair) => pair.startsWith(`${name}=`))
    ?.slice(name.length + 1);
  if (value) keep(name, value);
  return value || undefined;
}

/** Keeps `value` under `name`; none forgets it. */
export function keep(name: string, value?: string) {
  // biome-ignore lint/suspicious/noDocumentCookie: the Cookie Store API is async; the theme is read before the first paint.
  document.cookie = `${name}=${value ?? ''}; path=/; SameSite=Strict; max-age=${value ? age : 0}`;
}
