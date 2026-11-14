#!/bin/sh
# Fake headless executor: reads and discards the spec on stdin, writes a
# one-line receipt to the path given as $1, and exits 0. Used by
# executor-smoke tests to exercise the PASS classification path.
cat >/dev/null
echo "SMOKE_PASS" > "$1"
exit 0
