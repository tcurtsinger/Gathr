import type { WebTool } from '../api';

/** The link's host, as a row shows it. */
export function host(link: string): string {
  try {
    return new URL(link).host;
  } catch {
    return link;
  }
}

/** Where a tool is, as a row shows it: its link's host, or On this PC. */
export function where(tool: WebTool): string {
  return tool.kind === 'pc' ? 'On this PC' : host(tool.link);
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
    case 'starting':
      return { words: 'Starting' };
    case 'not_started':
      return { words: 'Didn’t start', tone: 'attention' };
    default:
      return { words: tool.kind === 'pc' ? 'Running' : 'Connected' };
  }
}
