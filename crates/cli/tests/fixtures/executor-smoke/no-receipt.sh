#!/bin/sh
# Fake headless executor: reads and discards the spec on stdin, exits 0
# without writing the expected receipt file. Exercises the NO_RECEIPT
# classification path (exit 0 alone must never be treated as PASS).
cat >/dev/null
exit 0
