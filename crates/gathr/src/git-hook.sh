#!/bin/sh
# Gathr's git hooks in an agent's worktree (0029), one script under each hook's name. A push
# goes through Gathr, whatever command ran it, which asks the person before a risky one; then, as
# every hook here, it runs the project's own hook of that name, if there is one.
name=$(basename "$0")
own=$(git config --get gathr.hooks)
if [ "$name" = pre-push ]; then
  refs=$(cat)
  if [ -z "$GATHR_AGENT_TOKEN" ] || [ -z "$GATHR_PUSH_URL" ]; then
    echo "Gathr: a push from an agent's worktree needs the person's OK, asked from its engine" >&2
    exit 1
  fi
  answer=$(printf '%s\n' "$refs" | curl -sS -X POST --data-binary @- \
    -H "Authorization: Bearer $GATHR_AGENT_TOKEN" -H "Content-Type: text/plain" \
    -H "Gathr-Remote: $1" -H "Gathr-Url: $2" "$GATHR_PUSH_URL")
  if [ "$answer" != allow ]; then
    echo "Gathr: the push wasn't allowed${answer:+ ($answer)}" >&2
    exit 1
  fi
  if [ -n "$own" ] && [ -x "$own/$name" ]; then
    printf '%s\n' "$refs" | "$own/$name" "$@"
    exit $?
  fi
  exit 0
fi
if [ -n "$own" ] && [ -x "$own/$name" ]; then
  exec "$own/$name" "$@"
fi
exit 0
