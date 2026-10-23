#!/usr/bin/env bash
set -euo pipefail

scripts=$(dirname "$(realpath "$0")")
sources=$(mktemp -d)
trap 'rm -rf "$sources"' EXIT
# These packages come from Ubuntu; runner-provided third-party feeds are unrelated.
# Keep the image's Ubuntu mirrors and signature configuration.
if [[ -f /etc/apt/sources.list.d/ubuntu.sources ]]; then
  cp /etc/apt/sources.list.d/ubuntu.sources "$sources/"
fi
options=(-o "Dir::Etc::sourceparts=$sources" -o Acquire::Retries=3
  -o Acquire::http::Timeout=30 -o Acquire::https::Timeout=30)
bash "$scripts/ci_retry.sh" sudo apt-get "${options[@]}" update --error-on=any
bash "$scripts/ci_retry.sh" sudo apt-get "${options[@]}" install -y "$@"
