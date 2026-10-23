#!/usr/bin/env bash
set -euo pipefail

for attempt in 1 2 3 4; do
  if "$@"; then
    exit 0
  else
    status=$?
  fi
  if ((attempt == 4)); then
    exit "$status"
  fi
  delay=$((attempt * 10))
  echo "Retrying in $delay seconds (attempt $attempt/4): $*" >&2
  sleep "$delay"
done
