# Doug ![cargo-badge](https://img.shields.io/crates/v/doug.svg)
> A time tracking command-line utility

## Why?

To have a time tracker that's not inhibited by [slow language startup times][0].

## Usage
```
USAGE:
    doug <SUBCOMMAND>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

SUBCOMMANDS:
    start      Track new or existing project
    status     Display elapsed time, start time, and running project name
    stop       Stop any running projects
    cancel     Stop running project and remove most recent time interval
    restart    Track last running project
    log        Display time intervals across all projects
    report     Display aggregate time from projects
    amend      Change name of currently running project
    edit       Edit last frame or currently running frame
    delete     Delete all intervals for project
```

### start
```
Track new or existing project

USAGE:
    doug start <project>

FLAGS:
    -h, --help    Prints help information

ARGS:
    <project>    project to track
```
### status
```
Display elapsed time, start time, and running project name

USAGE:
    doug status

FLAGS:
    -h, --help    Prints help information
```
### stop
```
Stop any running projects

USAGE:
    doug stop

FLAGS:
    -h, --help    Prints help information
```
### cancel
```
Stop running project and remove most recent time interval

USAGE:
    doug cancel

FLAGS:
    -h, --help    Prints help information
```
### restart
```
Track last running project

USAGE:
    doug restart

FLAGS:
    -h, --help    Prints help information
```
### log
```
Display time intervals across all projects

USAGE:
    doug log

FLAGS:
    -h, --help    Prints help information
```
### report
```
Display aggregate time from projects

USAGE:
    doug report

FLAGS:
    -h, --help    Prints help information
```
### amend
```
Change name of currently running project

USAGE:
    doug amend <project>

FLAGS:
    -h, --help    Prints help information

ARGS:
    <project>    new project name
```
### edit
```
Edit last frame or currently running frame

USAGE:
    doug edit [repo]

FLAGS:
    -h, --help    Prints help information

ARGS:
    <repo>    project to track
```
### delete
```
Delete all intervals for project

USAGE:
    doug delete <project>

FLAGS:
    -h, --help    Prints help information

ARGS:
    <project>    new project name
```
## Prior Art

- <http://wtime.sourceforge.net>
- <https://github.com/TailorDev/Watson>
- <https://github.com/danibram/time-tracker-cli>
- <https://github.com/samg/timetrap>

[0]: https://mail.python.org/pipermail/python-dev/2017-July/148656.html
