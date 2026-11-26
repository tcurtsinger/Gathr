import { Monitor, Pause, Play } from 'lucide-react';
import { useContext, useState } from 'react';
import { Button } from '@/components/ui/button';
import type { Failure } from '../api';
import { useCard } from '../components/Corner';
import { Face } from '../components/Face';
import { OpenPlugin } from '../plugins/Plugins';
import { clock } from './Room';
import { type ComputerStep, type ComputerUser, pauseComputer, useComputer, useShot } from './useComputer';
import './computer.css';

/** The driver's tools in words; any other is its own name. */
const DONE: Record<string, string> = {
  click: 'Clicked',
  double_click: 'Double-clicked',
  right_click: 'Right-clicked',
  type_text: 'Typed',
  press_key: 'Pressed',
  hotkey: 'Pressed',
  scroll: 'Scrolled',
  drag: 'Dragged',
  set_value: 'Set',
  launch_app: 'Opened',
  bring_to_front: 'Brought forward',
  get_window_state: 'Looked at',
  screenshot: 'Looked at',
};
const done = (tool: string) => DONE[tool] ?? tool.replaceAll('_', ' ').replace(/^./, (c) => c.toUpperCase());

/**
 * The Computer tab: Pause, for every agent's computer use, then each agent of this chat that used the computer, the one
 * that acted last first, with its latest picture of the window it works in and its steps. Off, it leads to Plugins.
 */
export function ComputerView({ chat, nameOf }: { chat: string; nameOf: (agent: string) => string }) {
  const computer = useComputer();
  const openPlugin = useContext(OpenPlugin);
  const [failure, setFailure] = useState<Failure>();
  useCard(failure);
  if (!computer) return null;
  const plugins = (
    <Button type="button" variant="outline" size="sm" onClick={() => openPlugin?.({ tool: computer.tool })}>
      Open Plugins
    </Button>
  );
  if (!computer.on || computer.driver === 'failed')
    return (
      <div className="filetab-none">
        <Monitor aria-hidden="true" />
        <strong>{computer.on ? "Computer use didn't start" : 'Computer use is off'}</strong>
        <span>{computer.why ?? 'Agents can use this PC once it is on in Plugins.'}</span>
        {plugins}
      </div>
    );
  if (computer.driver !== 'ready')
    return (
      <div className="filetab-none">
        <Monitor aria-hidden="true" />
        <strong>Getting computer use ready</strong>
        <span>Gathr is downloading the driver agents use the computer with.</span>
      </div>
    );
  const pause = (paused: boolean) => pauseComputer(paused).catch(setFailure);
  const here = computer.agents.filter((a) => a.chat === chat);
  return (
    <div className="cu">
      <div className="cu-bar" data-paused={computer.paused || undefined}>
        {computer.paused ? <Pause aria-hidden="true" /> : <span className="cu-live" />}
        <p>
          {computer.paused ? 'Paused' : 'Agents can use this PC'}
          <small>
            {computer.paused
              ? 'No agent uses it until you resume.'
              : "Pause stops every agent's computer use."}
          </small>
        </p>
        <Button type="button" variant="outline" size="sm" onClick={() => pause(!computer.paused)}>
          {computer.paused ? <Play /> : <Pause />}
          {computer.paused ? 'Resume' : 'Pause'}
        </Button>
      </div>
      {here.length ? (
        <div className="cu-scroll">
          {here.map((user) => (
            <User key={user.agent} user={user} name={nameOf(user.agent)} dim={computer.paused} />
          ))}
        </div>
      ) : (
        <div className="filetab-none">
          <strong>No agent here has used it yet</strong>
          <span>What an agent of this chat does on the computer shows here.</span>
        </div>
      )}
    </div>
  );
}

/** One agent: who and where, its latest picture with a mark where its last step landed, and its steps. */
function User({ user, name, dim }: { user: ComputerUser; name: string; dim: boolean }) {
  const url = useShot(user.shot);
  const [size, setSize] = useState<{ w: number; h: number }>();
  const mark = user.mark &&
    size && { left: `${(user.mark.x / size.w) * 100}%`, top: `${(user.mark.y / size.h) * 100}%` };
  return (
    <section className="cu-user">
      <div className="cu-who">
        <Face agent={{ id: user.agent, name }} size={20} />
        <strong>{name}</strong>
        {user.window && <span>· {user.window}</span>}
      </div>
      {url && (
        <div className="cu-shot" data-dim={dim || undefined}>
          <img
            src={url}
            alt={`What ${name} last saw of ${user.window ?? 'the window'}`}
            onLoad={(e) => setSize({ w: e.currentTarget.naturalWidth, h: e.currentTarget.naturalHeight })}
          />
          {mark && <i className="cu-mark" style={mark} />}
        </div>
      )}
      <ol className="cu-steps">
        {user.steps.map((s, i) => (
          // Steps have no id: their place in the last 50 is theirs while the list is shown.
          // biome-ignore lint/suspicious/noArrayIndexKey: as above
          <Step key={i} step={s} />
        ))}
      </ol>
    </section>
  );
}

function Step({ step }: { step: ComputerStep }) {
  const what = step.text ? `“${step.text}”` : step.target;
  const end = step.refused ? 'Paused' : step.failed ? `Didn't work: ${step.failed}` : null;
  return (
    <li data-off={end ? true : undefined}>
      <b>{done(step.tool)}</b>
      {what && <span>{what}</span>}
      {end && <em>{end}</em>}
      <time>{clock(step.at)}</time>
    </li>
  );
}
