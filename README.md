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



## Installation

If you haven't compiled Rust before, start by installing
[rustup](https://www.rustup.rs), and running `rustup install nightly` to install
the latest nightly version of Rust.

Then clone this repository, run `cargo +nightly build --release` and put the
generated binary `./target/release/eva` somewhere in your `PATH`.


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


## Acknowledgements

Many thanks go out to [Personal Productivity
@StackExchange](http://productivity.stackexchange.com), [zen
habits](http://zenhabits.net) and [GTD](http://gettingthingsdone.com) for
originally inspiring me! I also wouldn't have gotten this far without the
interesting discussions around scheduling algorithms with
[Myrjam](https://twitter.com/Myrjamvdv).
