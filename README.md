tuke
====

A proof of concept of **TU**I **KE**yboard for tmux.

![tuke.jpg](tuke.jpg)

How to run
----------

```console
$ cargo run

// or

$ cargo install --path .
$ tuke
```

Features
--------

- Software keyboard assumed to be running in a tmux pane
- Pressed keys are sent to other panes using `$ tmux send-keys` command
- Configurable key layout (see: [default-layout.jsonc](default-layout.jsonc))

Limitations
-----------

- Cannot use tmux full features (such as pop-up window)
- Cannot always show the cursor on the pane that the keys to be sent

