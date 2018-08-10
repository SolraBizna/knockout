This repository contains several components of a simple, but powerful, rsync-based incremental backup system.

# Requirements

**Client**:

- POSIX-compatible shell (such as `bash`)
- rsync
- SSH client

**Server**:

- Linux kernel (4.x or later strongly preferred)
- Lots of disk space on a `btrfs` volume
- rsync 3 or later
- SSH server
- Lua 5.3 (with LuaFileSystem and [`luaflock`](https://github.com/SolraBizna/luaflock))

Knockout can also act in a local mode, in which case an SSH server and client are not needed.

# Client

## Quick Setup

(See Per-Machine below for what needs to be done on the server side.)

Install rsync and git. On Debian or Ubuntu, you can do this with:

```sh
apt install rsync git
```

Clone this repository:

```sh
git clone https://github.com/SolraBizna/knockout
```

Choose a directory to store the Knockout client's configuration in. For user-driven backups, this should be `~/.knockout`. For root-driven backups, this should be either `~root/.knockout` or `/etc/knockout`. (If you want the configuration directory to be different from either of these, set `KNOCKOUT_DIR` in the environment when running the client.)

Create the following files in the directory:

- `host`: The IP address or hostname of the server running Knockout. (If this is `localhost`, Knockout will run in local mode, without having a remote shell layer in between.)
- `dir`: The path on the remote machine of this machine's backup directory (not including `current`).
- `sources`: The absolute paths of each top-level directory to back up. (Knockout will not cross filesystem boundaries by default, so it's normally safe to put `/` in here and not worry about it trying to back up `/proc`, etc.)
- `excludes`: Patterns for things that should not be included in the backup, whether to save space or for other reasons. (See "Include/Exclude Pattern Rules" in the [`rsync(1)` man page](https://linux.die.net/man/1/rsync) for syntax.)

The following files are optional:

- `extras`: Extra parameters to pass to `rsync`. You may want, for example, `--xattrs` to back up extended attributes (if your rsync version supports this), or `--acls` if you use ACLs, or `--no-one-file-system` if you have a complex hierarchy of filesystems you want to back up, you don't want to be bothered listing them in `sources`, and you intend to manually add `/proc` and friends to `excludes`.
- `rsh`: The command to use to log in to the remote machine. For example, if it's running its SSH server on a different port, something like: `ssh -p 8022`
- `vetted`: Patterns for files or directories that should definitely be in the backup. (Only used by `knockout-exclude-check` and not by the main client.)

Example:

```sh
mkdir ~/.knockout
cd ~/.knockout
echo 192.168.1.123 > host
echo /meat/backups/spunky > dir
cat > sources <<EOF
/home/rocko
/media/DogBowl
EOF
cat > excludes <<EOF
*~
.#*
\#*#
.cache
Cache
Downloads
nobackup
/media/DogBowl/Secret Documents
EOF
```

Do the initial backup:

```sh
/path/to/knockout/knockout-client.sh
```

(This will probably take a long time.)

Once the initial backup is complete and any errors are resolved, you may want to add a crontab entry to back up your machine automatically. (This requires passwordless key-based SSH access to the server; setting that up is outside the scope of this README.)

Example crontab entries:

```crontab
# Use Knockout to back up to 192.168.1.123 every two hours.
0 */2 * * * ping -qc1 -w 5 192.168.1.123 > /dev/null && ~/knockout/knockout-client.sh

# As above, but instead of having a passwordless SSH key, we have a setup that
# keeps ~/.ssh-agent up to date for an already-running, configured ssh-agent.
0 */2 * * * ping -qc1 -w 5 192.168.1.123 > /dev/null && nice -n 20 sh -c ". ~/.ssh-agent; export SSH_AUTH_SOCK; ~/knockout/knockout-client.sh"
```

## `knockout-exclude-check`

`knockout-exclude-check` is an optional tool that can help you decide what to exclude from your backup. It uses Rust, so make sure you install Rust before proceeding. ([Quick, easy installation intructions for Rust](https://www.rust-lang.org/en-US/install.html))

With Knockout configured and ready (but ideally before you perform your first backup), do something like the following:

```sh
cd ~/knockout/knockout-exclude-check
cargo run ~/Desktop/exclude-check.html
```

It will run, possibly for a very long time. Assuming there are no errors, it will create an HTML file at the given path. Open this HTML file in a modern web browser to see an interactive interface for deciding what to exclude (✗) and what should definitely be included (✓). Along the way, it will keep a running total of how much disk space is being taken up by files that are neither "excluded" nor "vetted". (I recommend stopping once that's down to a few gigabytes or so; the disk space savings from continuing past that point are outweighed by the time spent vetting every single little file.)

When you've finished, scroll to the bottom of the page to see the entries you should add to `excluded` and `vetted` in the Knockout configuration directory. Run `knockout-exclude-check` once more and refresh to make sure your changes stuck.

# Server

## Quick Setup

On Debian (and maybe Ubuntu), do:

```sh
apt install lua5.3 liblua5.3-dev luarocks lua-filesystem rsync openssh-server
luarocks install luaflock git
```

(If you already had `luarocks` installed, make sure it is configured for Lua version 5.3.)

Clone this repository somewhere world-readable:

```sh
git clone https://github.com/SolraBizna/knockout /opt/knockout
```

Put symlinks to `knockout-trim.lua` and `knockout-snap.sh` into appropriate places:

```sh
ln -s /opt/knockout/knockout-trim.lua /usr/local/sbin/knockout-trim
ln -s /opt/knockout/knockout-snap.sh /usr/local/bin/knockout-snap
```

Create a btrfs subvolume to store backups in.
```sh
btrfs subvolume create /meat/backups
```

Add entries like the following to root's crontab:

```crontab
### Knockout trimming

# On the hour, every hour: delete all but one backup per hour, keeping all
# backups made during this hour, last hour, or the hour before that.
  0   *   *   *   *   knockout-trim /meat/backups hour -p 2

# At 2:10AM every day: delete all but one backup per day, keeping all backups
# made today, yesterday, or the day before.
 10   2   *   *   *   knockout-trim /meat/backups day -p 2

# At 2:20AM on Sunday every week: delete all but one backup per week, keeping
# all backups made this week or last week.
 20   2   *   * Sun   knockout-trim /meat/backups week -p 1

# At 2:30AM on the first of every month: delete all but one backup per month,
# keeping all backups made this month or last month.
 30   2   1   *   *   knockout-trim /meat/backups month -p 1

# At 2:40AM on the first of every month: delete all but one backup per year,
# keeping all backups made this year or last year.
 40   2   1   *   *   knockout-trim /meat/backups year -p 1

# Fifty minutes after the hour, every hour: if the available space on the
# backup drive has fallen below 100000000 kilobytes (100GB), delete the oldest
# unimportant backup.
 50   *   *   *   *   if test $(df /meat/backups --output=avail | tail -n -1) -lt 100000000; then knockout-trim /meat/backups oldest; fi
```

(See the section on `knockout-trim` for more information.)

### Per-Machine

For each machine you want to back up, create a directory in your backup subvolume, create a `current` subvolume inside it, and set permissions appropriately.

Example, assuming that a user named `rocko` will be backing up a machine named `spunky`:

```sh
mkdir /meat/backups/spunky
btrfs subvolume create /meat/backups/spunky/current
chown -R rocko /meat/backups/spunky
chmod 700 /meat/backups/spunky
```

## `knockout-trim`

`knockout-trim` is the tool that trims "unimportant" backup snapshots, so that the farther back in time, the fewer backups are available. Usually, you will want to run it from cron. Have a look at the example crontab above for some basic examples, and run the command without any arguments for full usage information.

If you want a more (or less) complex backup hierarchy, use the `-d` option. Some examples, where the "base directory" passed to `knockout-trim` is `/meat/backups`:

- `-d 0`: A single machine being backed up on its own:  
  `/meat/backups`
- `-d 1`: (default) Multiple machines, sharing a backup drive:  
  `/meat/backups/heffer`  
  `/meat/backups/filburt`  
  `/meat/backups/spunky`
- `-d 2`: Multiple users, each with their own subdirectory of the backup drive:  
  `/meat/backups/rocko/heffer`  
  `/meat/backups/rocko/filburt`  
  `/meat/backups/rocko/spunky`  
  `/meat/backups/monty/hall`  
  `/meat/backups/monty/burns`  
  `/meat/backups/monty/scott`

If you have a **LOT** of machines being backed up, the fact that `knockout-trim` must lock each and every one may create a problem by running into the open files limit. If this is the case, you can use the `-l` option to change the "lock depth". By default, it's the same as `-d`; each machine contains its own lock file. In the `-d 2` example above, you could pass `-l 1` to have a lock file in each user's directory instead. Since **`knockout-snap` is not aware of this option**, if you do this, you must manually create a symlink in each machine's directory to the true lockfile location.

```sh
for MACHINE in /meat/backups/*/*; do
    if [ ! -L "$MACHINE" ]; then
        cd "$MACHINE"
        rm -f .lock
        ln -s ../.lock
    fi
done
```

`-g` and `-p` together specify the grace period during which backups are never trimmed. `-g` specifies a time unit, e.g. `day`. `-p` specifies how many time units into the past the grace period extends. (This starts to get inaccurate with extremely large time units and `-p` values, but a serious problem is seriously unlikely to result.)

- `-g day -p 0`: Never trim a backup made today.
- `-g day -p 1`: Never trim a backup made today or yesterday.
- `-g day -p 2`: Never trim a backup made today, yesterday, or the day before.

Note that, for example, `-g day -p 2` is *not* the same as `-g hour -p 48`.

# License

This repository is licensed under the zlib license. Basically, you can do what you like with it, except specifically take credit for writing it. See `LICENSE.md` for more information.
