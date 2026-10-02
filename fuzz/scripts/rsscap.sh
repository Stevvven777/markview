#!/usr/bin/env bash
# Run a command under a resident-set ceiling.
#
#   rsscap.sh <limit_mb> <cmd...>
#
# ASAN reserves a shadow-memory region of tens of terabytes, so the obvious
# `ulimit -v` cap aborts every sanitized binary before main. This watches the
# real resident size instead: past the ceiling the whole process *group* is
# signalled, which is the outcome the caller wants (a dead fuzzer, not a dead
# machine) and is reported distinctly so a campaign log can tell it apart from
# a crash.
#
# The limit is a backstop under libFuzzer's own `-rss_limit_mb`, which prints
# the offending input before exiting; this one only fires if that fails to.
#
# libFuzzer checks its limit *between* inputs, so one input that allocates
# heavily overshoots it; the watchdog is what actually bounds the process.
set -euo pipefail

limit_mb=${1:?usage: rsscap.sh <limit_mb> <cmd...>}
shift
[[ $# -gt 0 ]] || {
	echo "rsscap.sh: no command" >&2
	exit 2
}

"$@" &
child=$!

# Resident bytes of the child and every descendant, so a fuzz target that
# spawns workers (highlighting, image decode) is measured as a whole.
# `/proc` is walked rather than `ps --ppid`: the latter is a GNU extension
# whose argument parsing silently treats `--ppid N -p M` as one of the two,
# which made an earlier revision measure only part of the tree.
rss_kb() {
	local root=$1 total=0 pid kid r
	local queue=("$root")
	while ((${#queue[@]})); do
		pid=${queue[0]}
		queue=("${queue[@]:1}")
		r=$(awk '/^VmRSS:/ {print $2}' "/proc/$pid/status" 2>/dev/null || true)
		total=$((total + ${r:-0}))
		for kid in $(cat "/proc/$pid/task/$pid/children" 2>/dev/null || true); do
			queue+=("$kid")
		done
	done
	printf '%s' "$total"
}

watch() {
	local limit_kb=$((limit_mb * 1024)) total
	while kill -0 "$child" 2>/dev/null; do
		total=$(rss_kb "$child")
		if ((total > limit_kb)); then
			printf 'rsscap: pid %s exceeded %s MiB (%s KiB), killing\n' \
				"$child" "$limit_mb" "$total" >&2
			# Signal the numeric pid only. `kill -TERM -$child` assumed the
			# child led a process group, which it does not, so the fallback
			# ran instead and the intended group kill never happened.
			kill -TERM "$child" 2>/dev/null || true
			sleep 5
			kill -KILL "$child" 2>/dev/null || true
			return
		fi
		sleep 2
	done
}

watch &
watcher=$!

set +e
wait "$child"
status=$?
set -e
kill "$watcher" 2>/dev/null || true
wait "$watcher" 2>/dev/null || true
exit "$status"
