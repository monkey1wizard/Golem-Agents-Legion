#!/bin/sh
# Fake headless executor: never reads stdin and never exits on its own.
# Keep the hang in the shell process itself so timeout tests do not wait on
# a child process holding stdout/stderr pipes open after the shell is killed.
while :; do
  :
done
