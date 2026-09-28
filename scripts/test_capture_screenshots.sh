#!/usr/bin/env bash
# Exercise screenshot orchestration without a desktop or image processing.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
export CALLS=$work/calls ARGS=$work/args
export MARKVIEW=$work/markview
export PATH=$work:$PATH

cat >"$MARKVIEW" <<'SH'
#!/usr/bin/env bash
printf '%s\n' "$@" >>"$ARGS"
case "$BEHAVIOR" in
    fail) echo 'launch failed' >&2; exit 1 ;;
    timeout) exec /bin/sleep 30 ;;
    ready-exit) echo framebuffer; exit 1 ;;
esac
echo framebuffer
exec /bin/sleep 30
SH
cat >"$work/sleep" <<'SH'
#!/usr/bin/env bash
exec /bin/sleep 0.02
SH
cat >"$work/spectacle" <<'SH'
#!/usr/bin/env bash
echo capture >>"$CALLS"
SH
cat >"$work/python3" <<'SH'
#!/usr/bin/env bash
cat >/dev/null
SH
chmod +x "$MARKVIEW" "$work/sleep" "$work/spectacle" "$work/python3"

BEHAVIOR=ready bash "$root/scripts/capture_screenshots.sh" \
    en-structure zh-structure >"$work/log" 2>&1
test "$(wc -l <"$CALLS")" -eq 2
if grep -q -- '--scroll' "$ARGS"; then
    echo 'reader screenshots must not pass --scroll' >&2
    exit 1
fi
grep -q '/en/structure.md' "$ARGS"
grep -q '/zh/structure.md' "$ARGS"

for behavior in fail timeout ready-exit; do
    rm -f "$CALLS"
    if BEHAVIOR=$behavior bash "$root/scripts/capture_screenshots.sh" \
        en-structure >"$work/log" 2>&1; then
        echo "expected capture to fail for $behavior" >&2
        exit 1
    fi
    test ! -e "$CALLS"
    grep -q 'Markview exited or did not initialize' "$work/log"
    if [ "$behavior" = fail ]; then
        grep -q 'launch failed' "$work/log"
    fi
done
echo 'Screenshot orchestration tests passed.'
