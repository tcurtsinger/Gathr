#!/usr/bin/env bash
# Fails when a tracked source file is over the line limit in AGENTS.md.
set -euo pipefail

limit=400
status=0
while IFS= read -r file; do
  lines=$(wc -l < "$file")
  if [ "$lines" -gt "$limit" ]; then
    echo "$file: $lines lines (limit $limit)"
    status=1
  fi
done < <(git ls-files '*.rs' '*.ts' '*.tsx' '*.js' '*.mjs' '*.css' '*.sh')

exit $status
