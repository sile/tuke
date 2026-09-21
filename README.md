tuke
====

A proof of concept of **TU**I **KE**yboard: a software keyboard that drives
another process in a PTY.

![tuke.jpg](tuke.jpg)

tuke shows a program (the shell by default) in the top of the terminal and a
mouse-driven software keyboard below it. Clicking a key sends the corresponding
key to that program's PTY.

How to Run
----------

```console
$ cargo run

// or

$ cargo install --path .
$ tuke
```

Run a specific command instead of `$SHELL -l`:

```console
$ tuke --command 'vim'
```

Features
--------

- Embeds one child process in a PTY and drives it (no tmux required)
- Software keyboard that turns mouse clicks into key presses
- Configurable key layout (see: [layouts/default.jsonc](layouts/default.jsonc))

Limitations
-----------

- A single session only (no panes, windows, or sessions)
- The keyboard reserves the bottom rows of the terminal for itself
