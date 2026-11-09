#!/usr/bin/env bash
# Fails when a tracked source file is over the limits in AGENTS.md:
# 1,000 lines, and 200 characters a line. ui/src/registry is upstream code kept as published,
# held to it by scripts/registry.check.mjs instead (0030).
set -euo pipefail

status=0
while IFS= read -r file; do
  report=$(awk -v f="$file" '
    length($0) > 200 { long++; if (!first) first = NR }
    END {
      if (NR > 1000) printf "%s: %d lines (limit 1000)\n", f, NR
      if (long) printf "%s: %d lines over 200 characters, first at line %d\n", f, long, first
    }' "$file")
  if [ -n "$report" ]; then
    echo "$report"
    status=1
  fi
done < <(git ls-files '*.rs' '*.ts' '*.tsx' '*.js' '*.jsx' '*.mjs' '*.cjs' '*.css' '*.html' '*.sh' ':!ui/src/registry/')

exit $status
