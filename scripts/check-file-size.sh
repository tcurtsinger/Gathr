#!/usr/bin/env bash
# Fails when a tracked source file is over the limits in AGENTS.md:
# 400 lines (1,000 for design pages) and 200 characters a line.
set -euo pipefail

status=0
while IFS= read -r file; do
  limit=400
  case "$file" in designs/*.html) limit=1000 ;; esac
  report=$(awk -v f="$file" -v limit="$limit" '
    length($0) > 200 { long++; if (!first) first = NR }
    END {
      if (NR > limit) printf "%s: %d lines (limit %d)\n", f, NR, limit
      if (long) printf "%s: %d lines over 200 characters, first at line %d\n", f, long, first
    }' "$file")
  if [ -n "$report" ]; then
    echo "$report"
    status=1
  fi
done < <(git ls-files '*.rs' '*.ts' '*.tsx' '*.js' '*.jsx' '*.mjs' '*.cjs' '*.css' '*.html' '*.sh')

exit $status
