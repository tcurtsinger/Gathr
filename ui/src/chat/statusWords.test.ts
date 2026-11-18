import { expect, test } from 'vitest';
import type { Mark } from '../components/marks';
import { inWords, moving, parts, type Said } from './statusWords';

/**
 * "ann:working,rev:waiting@PR #18 checks" to a team: each agent's name, state and the one thing it waits on; "gathr" is
 * Gathr, said like an agent, last.
 */
function team(spec: string): Said[] {
  if (!spec) return [];
  return spec.split(',').map((p) => {
    const [who, rest] = p.split(':');
    const [state, on] = rest.split('@');
    return {
      name: who[0].toUpperCase() + who.slice(1),
      state: state as Mark,
      on,
      gathr: who === 'gathr' || undefined,
    };
  });
}

/** The line as plain text, its dots as "···". */
function line(spec: string): string {
  const t = team(spec);
  const said = parts(t)
    .map((p) => p.text)
    .join(', ');
  return said && `${said}${moving(t) ? ' ···' : ''}`;
}

test('the line’s words come from the team’s states: the status line page’s own table', () => {
  // The design's table (status-line/words.js), run on each team; "cant" is the design's "trouble".
  const table: [string, string][] = [
    ['', ''],
    ['ann:working', 'Ann is working ···'],
    ['ann:working,bo:working', 'Ann and Bo are working ···'],
    ['ann:working,bo:working,rev:working', '3 agents are working ···'],
    ['mira:working,ann:working,bo:working,rev:working', '4 agents are working ···'],
    ['ann:working,rev:waiting@PR #18 checks', 'Ann is working, Rev is waiting ···'],
    ['ann:working,bo:working,rev:waiting@PR #18 checks', '2 agents are working, 1 waiting ···'],
    ['bo:working,ann:waiting@PR #18 checks,rev:waiting@PR #18 checks', '1 agent is working, 2 waiting ···'],
    ['rev:waiting@PR #18 checks', 'Rev is waiting on PR #18 checks'],
    ['ann:waiting@PR #18 checks,rev:waiting@PR #18 checks', 'Ann and Rev are waiting on PR #18 checks'],
    ['ann:waiting@PR #18 checks,bo:waiting@Ann', 'Ann and Bo are waiting'],
    ['ann:waiting@PR #18 checks,bo:waiting@Ann,rev:waiting@PR #18 checks', '3 agents are waiting'],
    ['mira:needs', 'Mira needs you'],
    ['mira:needs,ann:working', 'Mira needs you, Ann is working ···'],
    ['mira:needs,bo:needs', 'Mira and Bo need you'],
    [
      'mira:needs,ann:working,bo:working,rev:waiting@PR #18 checks',
      'Mira needs you, 2 agents are working, 1 waiting ···',
    ],
    ['bo:cant,ann:working', 'Bo can’t run, Ann is working ···'],
    ['rev:stopped', 'Rev is stopped'],
    ['ann:working,bo:blocked', 'Ann is working, Bo is blocked ···'],
    ['ann:working,bo:working,rev:blocked', '2 agents are working, 1 blocked ···'],
    [
      'mira:needs,bo:cant,ann:working,rev:waiting@PR #18 checks',
      'Mira needs you, Bo can’t run, Ann is working, Rev is waiting ···',
    ],
    // Beyond the design's table: three who need you are counted, a wait on nothing named isn't said, Free never is.
    ['mira:needs,bo:needs,rev:needs', '3 agents need you'],
    ['rev:waiting', 'Rev is waiting'],
    ['ann:free,bo:free', ''],
    ['ann:free,rev:waiting@PR #18 checks', 'Rev is waiting on PR #18 checks'],
    // Gathr, said like an agent, last (the owner's pick, 2026-11-18): named beside names, never counted as an agent.
    ['gathr:waiting@PR #18 to land', 'Gathr is waiting on PR #18 to land'],
    ['hal:working,gathr:working', 'Hal and Gathr are working ···'],
    ['gathr:working,rue:waiting@PR #18 checks', 'Gathr is working, Rue is waiting ···'],
    ['ann:working,bo:working,gathr:working', '2 agents and Gathr are working ···'],
    ['a:working,b:working,c:working,d:working,e:working,gathr:working', '5 agents and Gathr are working ···'],
    [
      'mina:needs,ann:working,rev:waiting@PR #18 checks,bo:waiting@PR #19 checks,gathr:working',
      'Mina needs you, 1 agent and Gathr are working, 2 waiting ···',
    ],
    [
      'ann:working,bo:working,rev:waiting@PR #18 checks,gathr:waiting',
      '2 agents are working, 1 agent and Gathr waiting ···',
    ],
    ['ann:working,bo:working,rev:working,gathr:waiting', '3 agents are working, Gathr is waiting ···'],
    ['gathr:working,ann:waiting,bo:waiting', 'Gathr is working, 2 agents are waiting ···'],
  ];
  expect(table.map(([spec]) => line(spec))).toEqual(table.map(([, said]) => said));
});

test('a pull request’s wait reads as the approved pages write it; any other wait as the service says it', () => {
  const said = [
    'CI on #18',
    '#18 to merge or close',
    '#18 to be up to date',
    'CI on 0a1b2c3',
    'Made-up Lead',
    'a check',
  ];
  expect(said.map(inWords)).toEqual([
    'PR #18 checks',
    'PR #18 to merge',
    'PR #18 to be up to date',
    'CI on 0a1b2c3',
    'Made-up Lead',
    'a check',
  ]);
});
