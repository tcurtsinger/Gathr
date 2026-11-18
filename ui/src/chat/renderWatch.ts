/**
 * For tests: which messages React draws again. It installs React's developer-tools hook, which React calls after each
 * commit, so it must load before React does (import it first, for its effect). Each component that ran counts for the
 * message it sits in (`data-message-id`). A subtree React skipped keeps its fibers, and their flags, from when it last
 * ran: a component counts only when its fiber is not the one the commit before had (React swaps the two it keeps).
 */
interface Fiber {
  tag: number;
  flags: number;
  alternate: Fiber | null;
  child: Fiber | null;
  sibling: Fiber | null;
  return: Fiber | null;
  stateNode: { getAttribute?: (name: string) => string | null } | null;
}

const ran = new Map<string, number>();
let watching = false;
let before = new Set<Fiber>();
let now = new Set<Fiber>();
// Function, forwardRef, memo and simple memo components; PerformedWork is the flag of one that ran.
const COMPONENTS = new Set([0, 11, 14, 15]);
const PERFORMED_WORK = 1;

const messageOf = (fiber: Fiber) => {
  for (let f: Fiber | null = fiber; f; f = f.return) {
    const id = f.stateNode?.getAttribute?.('data-message-id');
    if (id) return id;
  }
  return null;
};

function count(fiber: Fiber | null) {
  for (let f = fiber; f; f = f.sibling) {
    if (COMPONENTS.has(f.tag)) {
      now.add(f);
      const id = watching && f.flags & PERFORMED_WORK && !before.has(f) ? messageOf(f) : null;
      if (id) ran.set(id, (ran.get(id) ?? 0) + 1);
    }
    count(f.child);
  }
}

(globalThis as { __REACT_DEVTOOLS_GLOBAL_HOOK__?: unknown }).__REACT_DEVTOOLS_GLOBAL_HOOK__ = {
  supportsFiber: true,
  renderers: new Map(),
  inject: () => 1,
  checkDCE: () => {},
  onScheduleFiberRoot: () => {},
  onCommitFiberRoot: (_id: number, root: { current: Fiber }) => {
    now = new Set();
    count(root.current.child);
    before = now;
  },
  onCommitFiberUnmount: () => {},
  onPostCommitFiberRoot: () => {},
};

/** Starts counting; the function it returns stops and gives each message's count. */
export function watchRenders() {
  ran.clear();
  watching = true;
  return () => {
    watching = false;
    return Object.fromEntries(ran);
  };
}
