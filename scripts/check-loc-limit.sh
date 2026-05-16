#!/usr/bin/env bash
set -euo pipefail

max_lines=700
failed=0

while IFS= read -r file; do
  lines=$(wc -l < "$file" | tr -d ' ')
  if [ "$lines" -gt "$max_lines" ]; then
    echo "$file has $lines lines; limit is $max_lines" >&2
    failed=1
  fi
done < <(find crates -type f -name '*.rs' \( -path '*/src/*' -o -path '*/tests/*' \) | sort)

exit "$failed"
