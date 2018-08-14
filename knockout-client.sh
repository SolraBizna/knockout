#!/bin/sh

set -e

ME_DIR=$(dirname "$0")
ME="$ME_DIR"/$(basename "$0")

if [ -z "$__KNOCKOUT_IS_LOCKED" ]; then
    export __KNOCKOUT_IS_LOCKED=1
    if [ -z "$KNOCKOUT_LOCK_PATH" ]; then
       if [ ! -z "$KNOCKOUT_DIR" -a -d "$KNOCKOUT_DIR" -a -w "$KNOCKOUT_DIR" ]; then
           KNOCKOUT_LOCK_PATH="$KNOCKOUT_DIR"/lock
       elif [ -d "$HOME"/.knockout -a -w "$HOME"/.knockout ]; then
           KNOCKOUT_LOCK_PATH="$HOME"/.knockout/lock
       elif [ -d "$HOME" -a -w "$HOME" ]; then
           KNOCKOUT_LOCK_PATH="$HOME"/.knockout_lock
       elif [ -d "$ME_DIR" -a -w "$ME_DIR" ]; then
           KNOCKOUT_LOCK_PATH="$ME_DIR"/.knockout_lock
       elif [ -w /tmp ]; then
           KNOCKOUT_LOCK_PATH="/tmp/.$(whoami)_knockout_lock"
       else
           echo "Unable to find a suitable place for putting the lock file."
           echo "Set KNOCKOUT_LOCK_PATH to a suitable directory in my environment."
           exit 1
       fi
    fi
    set +e
    flock -E42 -xn "$KNOCKOUT_LOCK_PATH" "$ME" "$@"
    WAT=$?
    if [ $WAT = 0 ]; then
        rm -f "$KNOCKOUT_LOCK_PATH"
    elif [ $WAT = 42 ]; then
        echo "Another Knockout client instance is already running, or the lock is stale."
        echo "If you're certain another client isn't running, delete $KNOCKOUT_LOCK_PATH and try again."
    fi
    exit $WAT
fi

if which ssh-agent >/dev/null; then
    KILL_AGENT=
    if [ -z "$SSH_AGENT_PID" -a -z "$SSH_AUTH_SOCK" ]; then
        exec env __KNOCKOUT_CALL_SSH_ADD=1 ssh-agent "$0" "$@"
    fi
    if [ ! -z "$__KNOCKOUT_CALL_SSH_ADD" ]; then
        ssh-add || true
    fi
fi

if [ -z "$KNOCKOUT_DIR" ]; then
    if [ -d "$HOME/.knockout" ]; then
        KNOCKOUT_DIR="$HOME/.knockout"
    else
        KNOCKOUT_DIR="/etc/knockout"
    fi
fi

if [ \! \( -r "$KNOCKOUT_DIR"/host -a -r "$KNOCKOUT_DIR"/dir -a -r "$KNOCKOUT_DIR"/sources -a -r "$KNOCKOUT_DIR"/excludes \) ]; then
    echo "Knockout client not fully configured. The following files need to be created:"
    [ -r "$KNOCKOUT_DIR"/host ] || echo "* $KNOCKOUT_DIR/host (destination host for this machine's backups)"
    [ -r "$KNOCKOUT_DIR"/dir ] || echo "* $KNOCKOUT_DIR/dir (destination dir on host for this machine's backups)"
    [ -r "$KNOCKOUT_DIR"/sources ] || echo "* $KNOCKOUT_DIR/sources (paths on this machine to back up)"
    [ -r "$KNOCKOUT_DIR"/excludes ] || echo "* $KNOCKOUT_DIR/excludes (passed to rsync --exclude-from)"
    exit 3
fi

if [ -r "$KNOCKOUT_DIR"/extras ]; then
    EXTRAS=$(cat "$KNOCKOUT_DIR"/extras)
else
    EXTRAS=
fi

if [ -t 1 -a "$#" -le 0 ]; then
    PROGRESS_OPTIONS="--human-readable --progress --stats"
else
    PROGRESS_OPTIONS=""
fi

HOST=$(cat "$KNOCKOUT_DIR"/host)
DIR=$(cat "$KNOCKOUT_DIR"/dir)
TARGET=
if [ "$HOST" = localhost ]; then
    TARGET="$DIR"/current
    RUN_COMMAND_ON_HOST=
    if [ $(whoami) = "root" ]; then
        NO_FAKE_SUPER=y
    fi
else
    TARGET="$HOST":"$DIR"/current
    if [ -z "$RSYNC_RSH" ]; then
        if [ -r "$KNOCKOUT_DIR"/rsh ]; then
            RSYNC_RSH=$(cat "$KNOCKOUT_DIR"/rsh)
        else
            RSYNC_RSH=ssh
        fi
        export RSYNC_RSH
    fi
    RUN_COMMAND_ON_HOST="$RSYNC_RSH $HOST"
fi

if [ -z "$NO_FAKE_SUPER" ]; then
    EXTRAS="-M--fake-super --numeric-ids $EXTRAS"
fi

rsync \
    $PROGRESS_OPTIONS \
    --acls \
    --archive \
    --chmod=u+rw \
    --delete-during \
    --delete-excluded \
    --exclude-from="$KNOCKOUT_DIR"/excludes \
    --files-from="$KNOCKOUT_DIR"/sources \
    --hard-links \
    --ignore-existing \
    --links \
    --one-file-system \
    --preallocate \
    --recursive \
    --sparse \
    --timeout=60 \
    $EXTRAS \
    "$@" \
    / \
    "$TARGET" \
    || exit 2

# $RUN_COMMAND_ON_HOST should not be quoted, as it may contain arguments
$RUN_COMMAND_ON_HOST knockout-snap "$DIR" || exit 5
