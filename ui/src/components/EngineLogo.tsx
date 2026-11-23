import { ClaudeLogo, OpenAILogo } from '@/components/assistant-ui/elements/logos';
import { ConnectMark } from './ConnectMark';

/**
 * The providers' logos, from assistant-ui's logos element, unchanged (designs/notices/engine-logos.txt): the one
 * place that knows whose logo is whose. Each is decoration beside a name already in words; the place sizes it.
 */
const LOGOS: Record<string, typeof ClaudeLogo> = { claude: ClaudeLogo, codex: OpenAILogo };

/** An engine's logo, by its id as the service names it; nothing for an engine it doesn't know. */
export function EngineLogo({ engine, className }: { engine: string; className?: string }) {
  const Logo = LOGOS[engine];
  return Logo ? <Logo className={className} /> : null;
}

/** An app outside Gathr, from the name it gave: its brand, and the engine whose logo it shows. */
function brandOf(app: string): { name: string; engine?: string } {
  if (/claude/i.test(app)) return { name: 'Claude', engine: 'claude' };
  if (/chatgpt|codex|openai/i.test(app)) return { name: 'ChatGPT', engine: 'codex' };
  return { name: app };
}

/** "Claude" or "ChatGPT" for an app of theirs; any other app keeps the name it gave. */
export const appName = (app: string) => brandOf(app).name;

/** The app's logo, or Connect's mark for an app that is neither. */
export function AppLogo({ app, className }: { app: string; className?: string }) {
  const { engine } = brandOf(app);
  if (!engine) return <ConnectMark className={className} />;
  return <EngineLogo engine={engine} className={className} />;
}
