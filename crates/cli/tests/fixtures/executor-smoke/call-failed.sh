#!/bin/sh
# Fake headless executor: reads and discards the spec on stdin, exits non-zero
# without writing a receipt. Exercises the CALL_FAILED classification path.
cat >/dev/null
exit 1
