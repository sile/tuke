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
- Host keys and pastes are forwarded to the child untouched

Limitations
-----------

- A single session only (no panes, windows, or sessions)
- The keyboard reserves the bottom rows of the terminal for itself
- The layout is chosen once at startup

Layouts
-------

A layout is a JSONC file that places keys at absolute coordinates inside the
keyboard region. Three are shipped:

| File | Size | Notes |
| --- | --- | --- |
| [layouts/default.jsonc](layouts/default.jsonc) | 144 cols | The full three-region keyboard |
| [layouts/compact-right.jsonc](layouts/compact-right.jsonc) | 144 cols | Same, narrower clusters |
| [layouts/mini.jsonc](layouts/mini.jsonc) | 69 cols | Fits in 80 columns; the keys a shell needs plus the arrows, no Shift/Alt |

The file to load is currently hard-coded, so `mini.jsonc` is not reachable yet.

To see what tuke reads out of a layout - every key's code, region, and label,
plus the keyboard's overall extent - run the [`inspect_layout`](examples/inspect_layout.rs)
example against it:

```console
$ cargo run --example inspect_layout layouts/mini.jsonc
```

That is also the worked example of reading a layout from Rust, and the extent
it prints is the number a layout chooser compares against the terminal size.

Roadmap
-------

What is planned next, in the order it is meant to happen.

### 1. Layout selection

Today the layout is loaded at startup and the terminal size only decides where
the keyboard is drawn, not which one. The next step is to let the terminal size
pick it. Three sizes exist; the small one must become reachable.

Agreed design:

- **Automatic switch (C).** A layout declares the smallest terminal it can be
  useful in, and tuke picks the largest layout that still fits. No key press is
  needed; resizing the terminal re-picks.
- **Manual switch (B).** A dedicated soft key cycles to the next layout, so the
  choice is possible at any size.
- **Minimize (A).** A dedicated soft key hides the keyboard and hands the whole
  terminal to the child, toggling back on a second press.

Open question to settle before coding: **how a switch key is written down.**
`KeyCode` today names either a character or a key that goes to the child
(`Char`, `Up`, `C-`, `Tab`, ...). A switch key names no child key at all, so it
needs a new kind of code (`LayoutNext` / `LayoutPrev` / `ToggleKeyboard`, or
equivalent) that `State` acts on instead of forwarding. That choice decides both
the JSONC syntax and how the core distinguishes "send this" from "do this".

### 2. Positional keyboard

A layout is currently a set of absolute coordinates, so an 80-column layout and
a 144-column layout cannot share one file and the keyboard can only be centred,
not aligned to where the hands are. A future format would place keys
positionally (rows of key widths) and let tuke compute the coordinates.

### 3. Notes

- The horizontal centring in [`src/geometry.rs`](src/geometry.rs) exists
  because layouts carry absolute columns; a positional format would make it
  unnecessary.
- `mini.jsonc` keys are five columns wide (only 69 of the 80 columns are
  used). `Esc`/`Tab` (9), `Ctrl` (9), `BSpace`/`Enter` (9) and the arrows (7)
  are wider so their labels fit and so the keys that are used most often are
  easier to hit.
