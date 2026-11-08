#!/bin/sh
# Runs a built Tinylogi.app in the foreground with debug logging. The installed background job
# is paused meanwhile so the two don't fight over the mouse; Ctrl-C stops it and resumes the job.
APP=$1
AGENT=gui/$(id -u)/net.caltona.tinylogi
PLIST=~/Library/LaunchAgents/net.caltona.tinylogi.plist
LOG=/tmp/tinylogi-dev.log

launchctl bootout "$AGENT" 2>/dev/null
: > $LOG
# Launch through LaunchServices so macOS checks the app's own permissions, not the terminal's.
open -n "$APP" --stdout $LOG --stderr $LOG --args --debug || exit 1

tail -f $LOG &
TAIL=$!

cleanup() {
    kill $TAIL
    pkill -INT -f "$APP/Contents/MacOS/tinylogi"
    sleep 1
    [ -f "$PLIST" ] && launchctl bootstrap "gui/$(id -u)" "$PLIST" && echo "resumed installed tinylogi"
    exit 0
}
trap cleanup INT TERM
# `wait` returns as soon as a trapped signal arrives, unlike a foreground command.
wait
