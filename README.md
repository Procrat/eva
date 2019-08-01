# Eva - Eva Virtual Assistant  [![Build Status](https://travis-ci.org/Procrat/eva.svg?branch=master)](https://travis-ci.org/Procrat/eva)

> Let algorithms decide your life.


## Disclaimer: work in progress

This project hasn't reached an alpha state yet. At the moment, it is just a tiny
CLI wrapper around a simple scheduling algorithm. Some people already find this
useful however, so maybe you do too!


## [Quick demo](https://procrat.github.io/eva-web)

The front end is developed in the [eva-web
project](https://github.com/Procrat/eva-web).


## Goal

Eva aims to be an opinionated application to manage your life when you don't
feel like doing that yourself. It will mainly do this by managing your todo list
as much as possible, for example by scheduling your tasks automatically so you
are saved from that mental burden.

It borrows from various productivity and motivation concepts like GTD, Pomodoro,
the Eisenhower scheme, flow, focussing on one task, small-chunking work, eating
the frog, etc.


## Core principles

1. Eva should be as **inobtrusive** as possible to maximise your flow, but
   **obtrusive** to manage your time.
2. Eva should maximise your **motivation** while minimising your time
   **procrastinating**.
3. **Automatic scheduling** should be left to machines since humans are better at
   deciding importance, deadlines and estimated duration of tasks then at
   actually scheduling all the things in their lives.
4. Your **mental health** is more important than your productivity.


## Installation

If you haven't built a Rust project before, start by installing
[rustup](https://www.rustup.rs), and running `rustup install nightly` to install
the latest nightly version of Rust. Second, add `$HOME/.cargo/bin` to your
`PATH`, e.g. by adding this line to your `~/.bashrc`:
```sh
export PATH="$PATH:$HOME/.cargo/bin"
```

Finally, to install Eva, clone this repository and run `cargo +nightly install`.


## Usage

`eva --help` will get you started.

```
$ eva --help
eva 0.0.1

USAGE:
    eva <SUBCOMMAND>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

SUBCOMMANDS:
    add         Adds a task
    help        Prints this message or the help of the given subcommand(s)
    rm          Removes a task
    schedule    Lets Eva suggest a schedule for your tasks
    set         Changes the deadline, duration, importance or content of an existing task
    tasks       Lists your tasks in the order you added them
```

```
$ eva help add
eva-add
Adds a task

USAGE:
    eva add <content> <deadline> <duration> <importance>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

ARGS:
    <content>       What is it that you want to do?
    <deadline>      When should it be finished? Give it in the format of '2 Aug 2017 14:03'.
    <duration>      How long do you estimate it will take? Give it in a (whole or decimal) number of hours.
    <importance>    How important is this task to you on a scale from 1 to 10?
```

```
$ date
Thu Aug  1 14:12:50 NZST 2019

$ eva add 'Think of plan to get rid of The Ring' '14 Aug 2019 00:00' 8 9

$ eva add 'Ask advice from Saruman' '10 Aug 2019 00:00' 8 4

$ eva add 'Visit Bilbo in Rivendel' '15 Aug 2019 00:00' 8 2

$ eva add 'Make some firework for the hobbits' '2 Aug 2019 18:00' 3 3

$ eva add 'Get riders of Rohan to help Gondor' '23 Aug 2019 00:00' 8 7

$ eva add 'Find some good pipe-weed' '4 Aug 2019 00:00' 1 8

$ eva add 'Go shop for white clothing' '4 Sep 2019 00:00' 2 3

$ eva add 'Prepare epic-sounding one-liners' '2 Aug 2019 19:00' 2 10

$ eva add 'Recharge staff batteries' '3 Aug 2019 00:00' 0.5 5

$ eva schedule
Schedule:
  Thu 1 Aug 14:23: 13. Prepare epic-sounding one-liners
    (deadline: Fri 2 Aug 19:00, duration: 2h0, importance: 10)
  Thu 1 Aug 16:23: 14. Recharge staff batteries
    (deadline: Sat 3 Aug 0:00, duration: 0h30, importance: 5)
  Fri 2 Aug 9:00: 9. Make some firework for the hobbits
   (deadline: Fri 2 Aug 18:00, duration: 3h0, importance: 3)
  Fri 2 Aug 12:00: 11. Find some good pipe-weed
    (deadline: Sun 4 Aug 0:00, duration: 1h0, importance: 8)
  Fri 2 Aug 13:00: 12. Go shop for white clothing
    (deadline: Wed 4 Sep 0:00, duration: 2h0, importance: 3)
  Sat 3 Aug 9:00: 7. Ask advice from Saruman
   (deadline: Sat 10 Aug 0:00, duration: 8h0, importance: 4)
  Sun 4 Aug 9:00: 6. Think of plan to get rid of The Ring
   (deadline: Wed 14 Aug 0:00, duration: 8h0, importance: 9)
  Mon 5 Aug 9:00: 8. Visit Bilbo in Rivendel
   (deadline: Thu 15 Aug 0:00, duration: 8h0, importance: 2)
  Tue 6 Aug 9:00: 10. Get riders of Rohan to help Gondor
    (deadline: Fri 23 Aug 0:00, duration: 8h0, importance: 7)
```


## Configuration

Eva Just Works™ without any extra configuration.

There are some things you could change if you really wanted to, by making a file
called `~/.config/eva/eva.toml` on GNU/Linux, `~/Library/Application
Support/eva/eva.toml` on Mac OS or
`C:\Users\<username>\AppData\Roaming\eva\eva.toml` on Windows. You can use
`~` and refer to environment variables if you want. These are the options you
can set at the moment, alongside their defaults:

```toml
# Which scheduling algorithm to use by default.
# This can be overridden with the --strategy flag to `eva schedule`
scheduling_strategy = "importance"

# Where Eva should store its SQLite database.
#   On GNU/Linux
database = "~/.local/share/eva/db.sqlite"
#   On Mac OS
database = "~/Library/Application Support/eva/db.sqlite"
#   On Windows
database = "C:\\Users\\<username>\\AppData\\Roaming\\eva\\db.sqlite"
```


## Roadmap

### v0.1 (short-term goals / MVP)

- [x] Task persistence
- [x] Minimal task management interface
- [x] Automatic scheduling
- [x] User configuration in CLI
- [x] Abstract configuration interface
- [x] [Web interface](https://github.com/Procrat/eva-web)
- [x] Time segmentation (e.g. sleep, working hours, morning rituals)
- [ ] Manual scheduling of fixed events
- [ ] Recurring events

### Next goals

- Task dependencies
- CalDAV / Google Calendar integration
  - Optional reminders
- Life organising scheme (values → life goals → projects → tasks)

### Possible future goals

- Blocked tasks (while waiting for something or someone)
- Time tracking + Pomodoro
- Regular, fixed moments of reflection (see [GTD](https://gettingthingsdone.com/what-is-gtd/)), both to ensure your task list still reflects reality, but also to be motivated by the work already done
- Way to make a brain dump and organise it later, at a fixed time
- Backup data to a server and possibly sync with Eva on other devices


## Acknowledgements

Many thanks go out to [Personal Productivity
@StackExchange](http://productivity.stackexchange.com), [Mark
Manson](https://markmanson.net), [zen habits](http://zenhabits.net) and
[GTD](http://gettingthingsdone.com) for originally inspiring me! I also wouldn't
have gotten this far without the interesting discussions around scheduling
algorithms with [Myrjam](https://twitter.com/Myrjamvdv).
