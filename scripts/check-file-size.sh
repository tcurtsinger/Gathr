#!/usr/bin/env bash
# Fails when a tracked source file is over the limits in AGENTS.md:
# 1,000 lines, and 200 characters a line. ui/src/registry is upstream code kept as published,
# held to it by scripts/registry.check.mjs instead (0030). Any tracked text file with a NUL byte
# fails too, and so does an AGENTS.md over 32,768 bytes.
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

# A NUL byte in a text file makes tools read it as binary and refuse it (S35). One perl for all.
nul=$(git ls-files -z '*.rs' '*.ts' '*.tsx' '*.js' '*.jsx' '*.mjs' '*.cjs' '*.css' '*.html' '*.sh' \
  '*.md' '*.json' '*.jsonl' '*.toml' '*.sql' '*.yml' '*.yaml' '*.txt' |
  xargs -0 perl -ne 'print "$ARGV: holds a NUL byte, first at line $.\n" if /\x00/; close ARGV if /\x00/ || eof')
if [ -n "$nul" ]; then
  echo "$nul"
  status=1
fi

# Every agent loads AGENTS.md whole in every session, so reference lives in the docs it links
# (#132). As staged, so a checkout's line endings don't count.
size=$(git cat-file -s :AGENTS.md)
if [ "$size" -gt 32768 ]; then
  echo "AGENTS.md: $size bytes (limit 32768). Move reference into the doc its index names for it."
  status=1
fi

exit $status
