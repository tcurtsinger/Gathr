import { createContext } from 'react';

/**
 * Opens a file a chat names in the chat's panel: a full path, or one in `agent`'s folder. Unset outside a chat,
 * where a file link shows as its words.
 */
export const OpenFile = createContext<((path: string, agent?: string) => void) | undefined>(undefined);

/**
 * The file a link's address names: a full path (`C:\…`, `C:/…`, `/Users/…`), a `file://` address, or a path in the
 * agent's folder, without a line (`:42`, `#L42`). None for a web address or a place in the page.
 */
export function linkedFile(url: string): string | undefined {
  if (url.startsWith('#') || url.startsWith('//')) return undefined;
  let path = url;
  if (/^file:/i.test(url)) {
    try {
      path = decodeURIComponent(url.replace(/^file:(\/\/)?/i, '')).replace(/^\/([A-Za-z]:)/, '$1');
    } catch {
      return undefined;
    }
  } else if (/^[a-z][a-z0-9+.-]+:/i.test(url)) return undefined;
  return path.replace(/#L\d.*$/, '').replace(/(:\d+){1,2}$/, '') || undefined;
}

/** What starts the line naming a file in a message's words (the service's `files::ATTACHED`). */
export const ATTACHED = 'Attached: ';
/** At most this many files in one message, each at most `MOST` bytes. */
export const MAX_FILES = 10;
export const MOST = 10 * 1024 * 1024;

const PICTURE = /\.(png|jpe?g|gif|webp)$/i;
/** A full path on Windows (`C:\…`, `C:/…`) or a Mac (`/Users/…`), never a URL. */
const LOCAL = /^(?:[A-Za-z]:[\\/]|\/(?!\/))/;
/**
 * A markdown image, read before markdown does: its path as written. Markdown would read a backslash before
 * punctuation as an escape and end the target at a space, so the target runs to its closing bracket.
 */
const IMAGE = /!\[[^\]\n]*\]\(\s*(?:<([^>\n]+)>|([^)\n]+?))\s*\)/g;

export const isPicture = (name: string) => PICTURE.test(name);
/** A Windows path, `C:\…` or `C:/…`: its case is ignored. */
export const onWindows = (path: string) => /^[A-Za-z]:/.test(path);
/** The path's parts, `.` and `..` followed. */
export function pathSteps(path: string) {
  const out: string[] = [];
  for (const s of path.split(/[\\/]/).filter(Boolean)) {
    if (s === '..') out.pop();
    else if (s !== '.') out.push(s);
  }
  return out;
}
export const fileName = (path: string) => path.split(/[\\/]/).pop() || path;
/** A path the one way, to compare: its case ignored on Windows. */
export const pathKey = (path: string) => {
  const joined = pathSteps(path).join('/');
  return onWindows(path) ? joined.toLowerCase() : joined;
};

/** A message's words in order: words, and runs of files where they stand. */
export type Part = { words: string } | { files: string[] };

/**
 * One reading of every message: each `Attached: <path>` line, and each markdown image of a local picture, becomes a
 * file where it stands; files with only space between them sit in one run. Inside a code block nothing is read.
 */
export function parts(text: string): Part[] {
  const out: Part[] = [];
  let words = '';
  let files: string[] = [];
  const flush = () => {
    if (files.length) {
      if (words.trim()) out.push({ words });
      out.push({ files });
      words = '';
      files = [];
    }
  };
  const said = (w: string) => {
    if (files.length && !w.trim()) return;
    flush();
    words += w;
  };
  const file = (path: string) => {
    if (!files.length && words.trim()) {
      out.push({ words });
      words = '';
    }
    files.push(path);
  };
  let fenced = false;
  for (const line of text.split(/(?<=\n)/)) {
    if (/^\s*(```|~~~)/.test(line)) fenced = !fenced;
    const attached = !fenced && line.startsWith(ATTACHED) && line.slice(ATTACHED.length).trim();
    if (attached) {
      file(attached);
      continue;
    }
    let from = 0;
    for (const hit of fenced ? [] : line.matchAll(IMAGE)) {
      const path = (hit[1] ?? hit[2]).trim();
      if (!LOCAL.test(path) || !isPicture(path)) continue;
      said(line.slice(from, hit.index));
      file(path);
      from = hit.index + hit[0].length;
    }
    said(line.slice(from));
  }
  flush();
  if (words.trim()) out.push({ words });
  return out;
}

/** The person's message: their words, and the files its lines name, which sit under the words. */
export function split(text: string): { words: string; files: string[] } {
  const files: string[] = [];
  const kept = text.split('\n').filter((line) => {
    const path = line.startsWith(ATTACHED) && line.slice(ATTACHED.length).trim();
    if (path) files.push(path);
    return !path;
  });
  return { words: files.length ? kept.join('\n').trimEnd() : text, files };
}
