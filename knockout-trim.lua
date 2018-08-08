#!/usr/bin/env lua5.3

if (1 << 31) < 0 or tostring((1 << 62) - 1) ~= "4611686018427387903" then
   print("Your Lua must be compiled with 64-bit integers to use knockout-trim")
   os.exit(1)
end

local s,lfs = pcall(require, "lfs")
if not s then
   print("LuaFileSystem is required. Please install it.")
   os.exit(1)
end

local s,flock = pcall(require, "flock")
if not s then
   print("The luaflock package is required. Please install it.")
   os.exit(1)
end

local function printf(format, ...) return print(format:format(...)) end

local me = arg[0] or "knockout-trim"

local function print_usage()
   printf([[

Usage: %s /base/path resolution [options]

By default, "/base/path" should be the path to the directory CONTAINING the
machines. For example, if the backup for machine "mybox" is in
"/meat/backups/mybox", then you should pass "/meat/backups" as the path. For
single-machine trims/backups, or more complex hierarchies, see the `-d` option.

"resolution" can be "oldest", in which case we will free up space by deleting
the oldest backup we can find. (We will never delete the last snapshot for a
machine.) Otherwise, it is one of the following time units: "minute", "hour",
"day", "week", "month", "quarter", "year", "decade", "century", "millenium".
The most recent backup in the given time unit will be preserved, and older ones
will be deleted. See the `-g` option.

Options:

-d DEPTH: (default 1) How many levels deep the backup snapshots are stored.
-l DEPTH: (default is whatever -d is set to) How many levels deep the lockfiles
  are. You only need this if you have a lot of machines. See the README.
-g GRACE: The grace period in which backups will never be trimmed. The default
  for "oldest" is "day", and the default for other resolutions is one time unit
  up from "resolution" (e.g. "week" for "day", "quarter" for "month").
-p PERIODS: (default 0) The number of grace periods into the past to preserve.
  The default of 0 means only backups in the current grace period (e.g. today)
  will be preserved. 1 also preserves the previous grace period, 2 preserves
  two periods before that, etc.
-v: Print extra information about what we're doing.
-n: Don't actually trim anything. Combine with -v to see what we would have
  done.
]], me)
end

local MINUTE_MUL = 100
local HOUR_MUL = MINUTE_MUL * 100
local DAY_MUL = HOUR_MUL * 100
local MONTH_MUL = DAY_MUL * 100
local YEAR_MUL = MONTH_MUL * 100

local time_periods = {}
time_periods.millenium = {
   to_bucket=function(datetime) return datetime // YEAR_MUL // 1000 end,
   from_bucket=function(datetime) return datetime * YEAR_MUL * 1000 end,
   len_for_grace=math.floor(1000*365.2422*24*60*60+0.5),
   format="%i",
}
time_periods.millenia = time_periods.millenium
time_periods.milleniums = time_periods.millenium -- ick
time_periods.millenium.default_grace = time_periods.millenium
time_periods.century = {
   to_bucket=function(datetime) return datetime // YEAR_MUL // 100 end,
   from_bucket=function(datetime) return datetime * YEAR_MUL * 100 end,
   len_for_grace=math.floor(100*365.2422*24*60*60+0.5),
   format="%i",
}
time_periods.century = time_periods.century
time_periods.century.default_grace = time_periods.millenium
time_periods.decade = {
   to_bucket=function(datetime) return datetime // YEAR_MUL // 10 end,
   from_bucket=function(datetime) return datetime * YEAR_MUL * 10 end,
   len_for_grace=math.floor(10*365.2422*24*60*60+0.5),
   format="%i",
}
time_periods.decades = time_periods.decade
time_periods.decade.default_grace = time_periods.century
time_periods.year = {
   to_bucket=function(datetime) return datetime // YEAR_MUL end,
   from_bucket=function(datetime) return datetime * YEAR_MUL end,
   len_for_grace=math.floor(365.2422*24*60*60+0.5),
   format="%i",
}
time_periods.years = time_periods.year
time_periods.year.default_grace = time_periods.century
time_periods.quarter = {
   to_bucket=function(datetime)
      local month = datetime // MONTH_MUL
      local year = month // 100
      month = month - (month - 1) % 3
      return month + year * 100
   end,
   from_bucket=function(datetime) return datetime * MONTH_MUL end,
   len_for_grace=math.floor(365.2422*24*60*60/4+0.5),
   format="%i.%02i",
}
time_periods.quarters = time_periods.quarter
time_periods.quarter.default_grace = time_periods.year
time_periods.month = {
   to_bucket=function(datetime) return datetime // MONTH_MUL end,
   from_bucket=function(datetime) return datetime * MONTH_MUL end,
   len_for_grace=math.floor(365.2422*24*60*60/12+0.5),
   format="%i.%02i",
}
time_periods.months = time_periods.month
time_periods.month.default_grace = time_periods.quarter
time_periods.week = {
   to_bucket=function(datetime)
      local day = datetime // DAY_MUL
      local month = day // 100
      day = day % 100
      local year = month // 100
      local month = month % 100
      local time = os.time({year=year,month=month,day=day})
      local date = os.date("*t", time)
      -- go to Sunday
      date.day = date.day - (date.wday - 1)
      date = os.date("*t", os.time(date))
      return date.year * 10000 + date.month * 100 + date.day
   end,
   from_bucket=function(datetime) return datetime * DAY_MUL end,
   len_for_grace=7*24*60*60,
   format="%i.%02i.%02i",
}
time_periods.weeks = time_periods.week
time_periods.week.default_grace = time_periods.month
time_periods.day = {
   to_bucket=function(datetime) return datetime // DAY_MUL end,
   from_bucket=function(datetime) return datetime * DAY_MUL end,
   len_for_grace=24*60*60,
   format="%i.%02i.%02i",
}
time_periods.days = time_periods.day
time_periods.day.default_grace = time_periods.week
time_periods.hour = {
   to_bucket=function(datetime) return datetime // HOUR_MUL end,
   from_bucket=function(datetime) return datetime * HOUR_MUL end,
   len_for_grace=60*60,
   format="%i.%02i.%02i-%02i",
}
time_periods.hours = time_periods.hours
time_periods.hour.default_grace = time_periods.day
time_periods.minute = {
   to_bucket=function(datetime) return datetime // MINUTE_MUL end,
   from_bucket=function(datetime) return datetime * MINUTE_MUL end,
   len_for_grace=60,
   format="%i.%02i.%02i-%02i%02i",
}
time_periods.minutes = time_periods.minute
time_periods.minute.default_grace = time_periods.hour
time_periods.second = {
   to_bucket=function(datetime) return datetime end,
   from_bucket=function(datetime) return datetime end,
   len_for_grace=1,
   format="%i.%02i.%02i-%02i%02i.%02i",
}
time_periods.seconds = time_periods.second
time_periods.second.default_grace = time_periods.minute

local DEFAULT_GRACE_FOR_OLDEST = time_periods.day

local depth = 1
local lock_depth = nil
local grace = nil
local grace_periods = 0
local verbose = false
local dry_run = false
local nonopt_arg = {}
local n = 1
local commandline_valid = true
while n <= #arg do
   if arg[n]:sub(1,1) == "-" then
      local optarg = arg[n]
      n = n + 1
      for m=2,#optarg do
         local c = optarg:sub(m,m)
         if c == "d" then
            local next = arg[n]
            n = n + 1
            if next == nil or next:match("[^0-9]")
            or #next < 1 or #next > 2 then
               print("Invalid argument to -d; expected a number between 0 and 99")
               commandline_valid = false
            else
               depth = assert(tonumber(next))
            end
         elseif c == "d" then
            local next = arg[n]
            n = n + 1
            if next == nil or next:match("[^0-9]")
            or #next < 1 or #next > 2 then
               print("Invalid argument to -l; expected a number between 0 and 99")
               commandline_valid = false
            else
               lock_depth = assert(tonumber(next))
            end
         elseif c == "g" then
            local next = arg[n]
            n = n + 1
            grace = time_periods[next]
            if not next then
               print("Invalid argment to -g; expected a time period such as \"minute\"")
               commandline_valid = false
            end
         elseif c == "p" then
            local next = arg[n]
            n = n + 1
            if next == nil or next:match("[^0-9]")
            or #next < 1 or #next > 9 then
               print("Invalid argument to -p; expected a number between 0 and 999999")
               commandline_valid = false
            else
               grace_periods = assert(tonumber(next))
            end
         elseif c == "v" then
            verbose = true
         elseif c == "n" then
            dry_run = true
         else
            commandline_valid = false
            printf("Unknown option: %q", c)
         end
      end
   else
      nonopt_arg[#nonopt_arg+1] = arg[n]
      n = n + 1
   end
end
if lock_depth and lock_depth > depth then
   print("-l cannot be greater than -d")
   commandline_valid = false
end
lock_depth = lock_depth or depth
local base, resolution
if commandline_valid then
   if #nonopt_arg < 2 then
      print("Need a base directory and a resolution")
      commandline_valid = false
   elseif #nonopt_arg > 2 then
      print("Too many non-option arguments")
      commandline_valid = false
   else
      base = nonopt_arg[1]
      if nonopt_arg[2] == "oldest" then
         resolution = "oldest"
         if not grace then grace = DEFAULT_GRACE_FOR_OLDEST end
      elseif time_periods[nonopt_arg[2]] then
         resolution = time_periods[nonopt_arg[2]]
         if not grace then grace = resolution.default_grace end
      else
         print("Resolution must be \"oldest\" or a time period such as \"minute\"")
         commandline_valid = false
      end
   end
end
if not commandline_valid then
   print_usage()
   os.exit(1)
end

if dry_run then
   os.execute = function() return true end
end

local exec
if verbose then
   function exec(format, ...)
      local cmdline = format:format(...)
      io.write("\x1B[32m$ ",cmdline,"\x1B[0m\n")
      assert(os.execute(cmdline))
   end
else
   function exec(format, ...)
      assert(os.execute(format:format(...)))
   end
end

local function parse_datetime(x)
   local year, month, day, hour, minute, second
   year, x = x:match("^([0-9]+)(.*)$")
   if not year or #year > 8 then return nil end
   year = assert(tonumber(year))
   if year < 1 then return nil end -- years start at 1, silly
   if x ~= "" then
      month, x = x:match("^%.([0-9][0-9])(.*)$")
      if not month then return nil end
      month = assert(tonumber(month))
      if month < 1 or month > 12 then return nil end
      if x ~= "" then
         day, x = x:match("^%.([0-9][0-9])(.*)$")
         if not day then return nil end
         day = assert(tonumber(day))
         if day < 1 or day > 31 then return nil end
         if x ~= "" then
            hour, x = x:match("^%-([0-9][0-9])(.*)$")
            if not hour then return nil end
            hour = assert(tonumber(hour))
            -- allow both 0 and 24... stupid airports
            if hour < 0 or hour > 24 then return nil end
            if x ~= "" then
               minute, x = x:match("^([0-9][0-9])(.*)$")
               if not minute then return nil end
               minute = assert(tonumber(minute))
               if minute < 0 or minute > 59 then return nil end
               if x ~= "" then
                  second, x = x:match("^%.([0-9][0-9])(.*)$")
                  if not second then return nil end
                  second = assert(tonumber(second))
                  if second < 0 or second > 61 then return nil end
               end
            end
         end
      end
   end
   month = month or 0
   day = day or 0
   hour = hour or 0
   minute = minute or 0
   second = second or 0
   return year * YEAR_MUL + month * MONTH_MUL + day * DAY_MUL
      + hour * HOUR_MUL + minute * MINUTE_MUL + second
end

local function time_to_datetime(time)
   local date = os.date("*t", time)
   return date.year * YEAR_MUL + date.month * MONTH_MUL + date.day * DAY_MUL
      + date.hour * HOUR_MUL + date.min * MINUTE_MUL + date.sec
end

local now = os.time()
local grace_time = time_to_datetime(now - grace.len_for_grace * grace_periods)
now = time_to_datetime(now)

local machines = {}
local snaps = {}
local locks = {}
local function descend(base, depth, lock_depth)
   assert(depth >= 0)
   if lock_depth == 0 then
      local f = assert(io.open(base.."/.lock", "a+"))
      local s,e = flock(f)
      if not s then
         printf("flock(%s/.lock): %s", base, e)
         os.exit(1)
      end
      locks[#locks+1] = f
   end
   if depth == 0 then
      local machine = {path=base, snapcount=0}
      for entry in lfs.dir(base) do
         local when = parse_datetime(entry)
         if when and when < grace_time then
            local snap = {name = entry,
                          machine = machine,
                          when = when}
            machine.snapcount = machine.snapcount + 1
            snaps[#snaps+1] = snap
         end
      end
      machines[base] = machine
   else
      for subdir in lfs.dir(base) do
         if subdir ~= "lost+found" and subdir:sub(1,1) ~= "." then
            descend(base .. "/" .. subdir, depth - 1, lock_depth - 1)
         end
      end
   end
end
descend(base, depth, lock_depth)

local function compare_snap(a, b)
   if a.when < b.when then return true
   elseif a.machine.path < b.machine.path then return true
   else return false
   end
end

local function quote_for_shell(q)
   return q:gsub("'", "'\\''")
end

if resolution == "oldest" then
   -- Delete the oldest snap that is not the last snap for its machine
   local oldest
   for _, snap in ipairs(snaps) do
      if snap.machine.snapcount > 1 then
         if not oldest or compare_snap(snap, oldest) then
            oldest = snap
         end
      end
   end
   if not oldest then
      print("Unable to find a snapshot we could safely delete!")
      os.exit(1)
   end
   exec("btrfs subvolume delete '%s' > /dev/null",
        quote_for_shell(oldest.machine.path .. "/" .. oldest.name))
else
   -- Put each snap into the correct bucket for its timestamp
   for _, snap in ipairs(snaps) do
      local machine = snap.machine
      local bucket_key = resolution.to_bucket(snap.when)
      machine.buckets = machine.buckets or {}
      if machine.buckets[bucket_key] then
         table.insert(machine.buckets[bucket_key], snap)
      else
         machine.buckets[bucket_key] = {snap}
      end
   end
   -- For each bucket, delete all snaps in the bucket except the newest one,
   -- then possibly rename the newest.
   for _, machine in pairs(machines) do
      for _, bucket in pairs(machine.buckets) do
         table.sort(bucket, compare_snap)
         for n=1, #bucket - 1 do
            exec("btrfs subvolume delete '%s' > /dev/null",
                 quote_for_shell(machine.path .. "/" .. bucket[n].name))
         end
         local snap = bucket[#bucket]
         local second = snap.when % 100
         local minute = (snap.when // MINUTE_MUL) % 100
         local hour = (snap.when // HOUR_MUL) % 100
         local day = (snap.when // DAY_MUL) % 100
         local month = (snap.when // MONTH_MUL) % 100
         local year = snap.when // YEAR_MUL
         local new_name = resolution.format:format(year, month, day,
                                                   hour, minute, second)
         if #new_name < #snap.name then
            exec("mv '%s' '%s'",
                 quote_for_shell(snap.machine.path .. "/" .. snap.name),
                 quote_for_shell(snap.machine.path .. "/" .. new_name))
         end
      end
   end
end
