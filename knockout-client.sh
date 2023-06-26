#!/usr/bin/env bash

set -e

ME_DIR="$(dirname "$0")"
ME="$ME_DIR/$(basename "$0")"

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

HOST="$(cat "$KNOCKOUT_DIR"/host)"

if [ "$HOST" != localhost -a ! -f "$KNOCKOUT_DIR/no-ssh-agent" ] && \
       which ssh-agent >/dev/null; then
    KILL_AGENT=
    if [ -z "$SSH_AGENT_PID" -a -z "$SSH_AUTH_SOCK" ]; then
        exec ssh-agent "$0" "$@"
    fi
    try_ssh_add () {
        KEY="$1"
        FINGERPRINT="$(ssh-keygen -l -f "$KEY" | awk '{print $2}')"
        if ! ssh-add -l | grep -qFe "$FINGERPRINT"; then
            if [ -t 2 ]; then
                # stderr is a tty, the user may care which key(s) got used
                ssh-add "$KEY"
            else
                # stderr is not a tty, we're probably a cron job, don't be
                # verbose
                ssh-add -q "$KEY"
            fi
        fi
    }
    if [ -f "$KNOCKOUT_DIR/no-ssh-add" ]; then
        true # ssh-add has been suppressed, do nothing
    elif [ -f "$HOME/.ssh/id_knockout" ]; then
        try_ssh_add "$HOME/.ssh/id_knockout"
    elif ! compgen -G "$HOME/.ssh/id_*.pub" >/dev/null; then
        echo "You don't have any SSH keys I could find. To avoid having to enter your"
        echo "password twice, I recommend creating an SSH key for use with Knockout."
        echo
        echo "For example:"
        echo
        echo "    ssh-keygen -t ed25519 -f ~/.ssh/id_knockout"
        echo
        echo "You will then have to add the public key (from ~/.ssh/id_knockout.pub) to"
        echo "the ~/.ssh/authorized_keys file on your Knockout server."
        echo
    else
        echo "You don't have a Knockout-specific SSH key, so I'm trying all the SSH keys I"
        echo "can find."
        echo
        echo "To avoid this message in the future, and potentially some unwanted password"
        echo "prompts, make a Knockout-specific private key, either by generating it with:"
        echo
        echo "    ssh-keygen -t ed25519 -f ~/.ssh/id_knockout"
        echo
        echo "or by copying an existing key into place. Either way, make sure that the"
        echo "corresponding public key is in the ~/.ssh/authorized_keys file on your"
        echo "Knockout server."
        echo
        for PUBKEY in "$HOME/.ssh/id_"*".pub"; do
            KEY="$(echo "$PUBKEY" | sed -Ee 's/\.pub$//')"
            try_ssh_add "$KEY"
        done
    fi
fi

if [ -r "$KNOCKOUT_DIR"/extras ]; then
    EXTRAS="$(cat "$KNOCKOUT_DIR"/extras)"
else
    EXTRAS=
fi

if [ -t 1 -a "$#" -le 0 ]; then
    PROGRESS_OPTIONS="--human-readable --progress --stats"
    WE_ARE_LOUD=yes
else
    PROGRESS_OPTIONS=""
    WE_ARE_LOUD=no
fi

DIR="$(cat "$KNOCKOUT_DIR"/dir)"
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
            RSYNC_RSH="$(cat "$KNOCKOUT_DIR"/rsh)"
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

if rsync \
    --rsync-path "nice -n 20 rsync" \
    $PROGRESS_OPTIONS \
    --acls \
    --archive \
    --chmod=u+rw \
    --delete-during \
    --delete-excluded \
    --exclude-from="$KNOCKOUT_DIR"/excludes \
    --files-from="$KNOCKOUT_DIR"/sources \
    --hard-links \
    --one-file-system \
    --recursive \
    --sparse \
    --timeout=60 \
    $EXTRAS \
    "$@" \
    / \
    "$TARGET"
then
    # $RUN_COMMAND_ON_HOST should not be quoted, as it may contain arguments
    $RUN_COMMAND_ON_HOST knockout-snap "$DIR" || exit 5
    if [ -t 1 ]; then
        echo
        case "$TERM" in
            screen* | xterm* | tmux* | vt1* | ansi*)
                # Any of these terminals should be able to cope with these
                # ANSI codes. Even if they're not color-capable, they'll
                # hopefully ignore the color code and respect only the bold
                # code... or, at the very worst, just eat the whole escape
                # sequence.
                printf "\e[32;1mBackup completed successfully.\e[0m\n"
                ;;
            *)
                echo "Backup completed successfully."
                ;;
        esac
        echo
    fi
else
    echo
    if [ ! -t 1 ]; then
        TERM=nope
    fi
    case "$TERM" in
        screen* | xterm* | tmux* | vt1* | ansi*)
            printf "\e[31;1mBackup was NOT completed successfully!\e[0m\n"
            ;;
        *)
            echo "Backup was NOT completed successfully!"
            ;;
    esac
    echo
    echo "Please resolve any error messages above and run the backup again."
    if [ "$WE_ARE_LOUD" = "yes" ]; then
        echo
        echo "To print less information, making it easier to see what went wrong, pass a"
        echo "single -- as an argument, like:"
        echo
        echo "    $0 --"
    fi
    echo
    exit 2
fi
