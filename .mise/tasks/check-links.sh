#!/usr/bin/env bash

# Find all markdown files, excluding:
# - node_modules directory
# - target directory (Rust build output)
# - docs directory (has its own link checker)
# - hidden directories (starting with .)

find . -type f -name "*.md" \
  -not -path "*/node_modules/*" \
  -not -path "*/target/*" \
  -not -path "*/docs/*" \
  -not -path "*/.*/*" \
  -exec lychee --retry-wait-time 30 --max-retries 5 --exclude-all-private {} +
