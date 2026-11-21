import type { WebTool } from '../api';

/** The link's host, as a row shows it. */
export function host(link: string): string {
  try {
    return new URL(link).host;
  } catch {
    return link;
  }
}

/** A tool's state in a word or two, and its tone: muted, attention or danger. */
export function stateOf(tool: WebTool): { words: string; tone?: 'attention' | 'danger' } {
  if (!tool.enabled) return { words: 'Disabled' };
  switch (tool.state) {
    case 'key_refused':
      return { words: 'Key refused', tone: 'attention' };
    case 'sign_in':
      return { words: 'Sign in again', tone: 'attention' };
    case 'not_reachable':
      return { words: 'Not reachable', tone: 'danger' };
    default:
      return { words: 'Connected' };
  }
}
