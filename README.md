# Rus
> A time tracking command-line utility

## Why?

Python, along with many other interpreted languages, has a [slow startup
time][0]. A Rust based time tracker can avoid this issue.

## Usage

- `start  [project-name]`
  start new project or existing project, if no project name provided use
  existing one or error. If currently running project, stop
  it and start new one.
- `status`
  displays current elapsed time, start time, and project
- `stop`
  stops any running projects
- `log`
  displays time intervals across all projects
- `report`
  displays aggregate time for the last week
  + flags for start and end dates
  + flags for year, month, day
  + flags for filtering by project, tag
- `cancel`
  stops running project and removes most recent time interval
- `restart`
  restarts currently tracking project (if no currently running project, it
  starts last running project [if there was one])
- `amend`
  change project name of currently running period
- `edit`
  last frame or currently running frame raw (optionally specify frame id or
  position)

## Previous Work

- http://wtime.sourceforge.net
- https://github.com/TailorDev/Watson
- https://github.com/danibram/time-tracker-cli
- https://github.com/samg/timetrap

[0]: https://mail.python.org/pipermail/python-dev/2017-July/148656.html
