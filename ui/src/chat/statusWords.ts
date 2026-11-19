/**
 * A pull request's wait in the approved pages' words: "PR #18 checks", "PR #18 to merge", "PR #18 to land". The
 * service names them "CI on #18", "#18 to merge or close" (`tasks::wait_name`) and "#18 to land", as agents read
 * them through the `waiting` tool; anything else is said as the service says it.
 */
export function inWords(what: string): string {
  return what
    .replace(/^CI on #(\d+)$/, 'PR #$1 checks')
    .replace(/^#(\d+) to merge or close$/, 'PR #$1 to merge')
    .replace(/^#(\d+) to land$/, 'PR #$1 to land')
    .replace(/^#(\d+) to be up to date$/, 'PR #$1 to be up to date');
}
