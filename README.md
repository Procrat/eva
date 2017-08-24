# Eva - Eva Virtual Assistant

Let algorithms decide your life.


## Disclaimer: work in progress

This project hasn't reached an alpha state yet. At the moment, it is just a tiny
CLI wrapper around a simple scheduling algorithm. Some people already find this
useful however, so maybe you do too!


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
3. **Automatic scheduling** should left to machines since humans are better at
   deciding importance, deadlines and estimated duration of tasks then at
   actually scheduling all the things in their lives.


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
    <deadline>      When should it be finished? Give it in the format of '2 Aug
                    2017 14:03'.
    <duration>      How long do you estimate it will take? Give it in a (whole or
                    decimal) number of hours.
    <importance>    How important is this task to you on a scale from 1 to 10?
```

```
$ date
Mon Aug 21 08:00:00 NZST 2017

$ eva add 'Think of plan to get rid of The Ring' '3 Sep 2017 00:00' 48 9

$ eva add 'Ask advice from Saruman' '30 Aug 2017 00:00' 48 4

$ eva add 'Visit Bilbo in Rivendel' '4 Sep 2017 00:00' 48 2

$ eva add 'Make some firework for the hobbits' '22 Aug 2017 18:00' 3 3

$ eva add 'Get riders of Rohan to help Gondor' '12 Sep 2017 00:00' 72 7

$ eva add 'Find some good pipe-weed' '24 Aug 2017 00:00' 1 8

$ eva add 'Go shop for white clothing' '24 Sep 2017 00:00' 2 3

$ eva add 'Prepare epic-sounding one-liners' '22 Aug 2017 19:00' 2 10

$ eva add 'Recharge staff batteries' '23 Aug 2017 00:00' 0.5 5

$ eva schedule
Tasks:
  1. Think of plan to get rid of The Ring
    (deadline: Sun 3 Sep 0:00, duration: 48h0, importance: 9)
  2. Ask advice from Saruman
    (deadline: Wed 30 Aug 0:00, duration: 48h0, importance: 4)
  3. Visit Bilbo in Rivendel
    (deadline: Mon 4 Sep 0:00, duration: 48h0, importance: 2)
  4. Make some firework for the hobbits
    (deadline: Tue 22 Aug 18:00, duration: 3h0, importance: 3)
  5. Get riders of Rohan to help Gondor
    (deadline: Tue 12 Sep 0:00, duration: 72h0, importance: 7)
  6. Find some good pipe-weed
    (deadline: Thu 24 Aug 0:00, duration: 1h0, importance: 8)
  7. Go shop for white clothing
    (deadline: Sun 24 Sep 0:00, duration: 2h0, importance: 3)
  8. Prepare epic-sounding one-liners
    (deadline: Tue 22 Aug 19:00, duration: 2h0, importance: 10)
  9. Recharge staff batteries
    (deadline: Wed 23 Aug 0:00, duration: 0h30, importance: 5)

Schedule:
  Sun 20 Aug 20:00: 8. Prepare epic-sounding one-liners
    (deadline: Tue 22 Aug 19:00, duration: 2h0, importance: 10)
  Sun 20 Aug 22:00: 6. Find some good pipe-weed
    (deadline: Thu 24 Aug 0:00, duration: 1h0, importance: 8)
  Sun 20 Aug 23:00: 9. Recharge staff batteries
    (deadline: Wed 23 Aug 0:00, duration: 0h30, importance: 5)
  Sun 20 Aug 23:30: 4. Make some firework for the hobbits
    (deadline: Tue 22 Aug 18:00, duration: 3h0, importance: 3)
  Mon 21 Aug 2:30: 1. Think of plan to get rid of The Ring
    (deadline: Sun 3 Sep 0:00, duration: 48h0, importance: 9)
  Wed 23 Aug 2:30: 5. Get riders of Rohan to help Gondor
    (deadline: Tue 12 Sep 0:00, duration: 72h0, importance: 7)
  Sat 26 Aug 2:30: 2. Ask advice from Saruman
    (deadline: Wed 30 Aug 0:00, duration: 48h0, importance: 4)
  Mon 28 Aug 2:30: 7. Go shop for white clothing
    (deadline: Sun 24 Sep 0:00, duration: 2h0, importance: 3)
  Mon 28 Aug 4:30: 3. Visit Bilbo in Rivendel
    (deadline: Mon 4 Sep 0:00, duration: 48h0, importance: 2)
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
- [x] User configuration
- [ ] Manual scheduling of fixed events
- [ ] Recurring events
- [ ] Take fixed hours of sleeping and/or working into account

### Next goals

- CalDAV / Google Calendar integration
  - Optional reminders
- Life organising scheme (values → life goals → projects → tasks)

### Unprioritised goals

- Bulk-edit tasks
- Integration with desktop environment
- Time tracking + Pomodoro
- "Forced" reflection
- Web interface
- Interactive terminal UI (possibly reusing [Procrat/eva-deprecated](https://github.com/Procrat/eva-deprecated), possibly with `ncurses`)
- Scratchpad
- Laudatory diary
- Frozen tasks
- API/hooks for easy extension
- Integration with TaskWarrior (if at all possible)


## Acknowledgements

Many thanks go out to [Personal Productivity
@StackExchange](http://productivity.stackexchange.com), [zen
habits](http://zenhabits.net) and [GTD](http://gettingthingsdone.com) for
originally inspiring me! I also wouldn't have gotten this far without the
interesting discussions around scheduling algorithms with
[Myrjam](https://twitter.com/Myrjamvdv).
