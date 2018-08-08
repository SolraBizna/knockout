#!/bin/sh

set -e

if [ $# -ne 1 ]; then
    echo "Usage: $0 /base/machine"
    echo "e.g. $0 /meat/backups/mybox"
    exit 1
fi

if [ ! -e "$1/current" ]; then
    echo "$0: current directory didn't exist, doing nothing"
    exit 0
fi

ME_DIR=$(dirname "$0")
ME="$ME_DIR"/$(basename "$0")

if [ -z "$__KNOCKOUT_IS_LOCKED" ]; then
    export __KNOCKOUT_IS_LOCKED=1
    flock -E42 -w60 -x "$1"/.lock "$ME" "$@"
    WAT=$?
    if [ $WAT = 42 ]; then
        echo "Unable to lock the backup directory after 60 seconds. This shouldn't happen."
        echo "If you're ABSOLUTELY certain this is in error, delete the `.lock` file."
    fi
    exit $WAT
fi

while true; do
    NOW=$(date +%Y.%m.%d-%H%M.%S)
    if [ -e "$1/$NOW" ]; then
        # this should be incredibly rare, but it's worth handling
        # theroetically this handling method opens up a DOS, but...
        sleep 1
    else
        break
    fi
done

btrfs subvolume snapshot -r "$1/current" "$1/$NOW"
