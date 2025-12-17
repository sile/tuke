tuke
====

A proof of concept of **TU**I **KE**yboard for tmux.

![tuke.jpg](tuke.jpg)

How to Run
----------

```console
$ cargo run

// or

$ cargo install --path .
$ tuke
```

Features
--------

- Software keyboard designed to run in a tmux pane
- Pressed keys are sent to other panes using the `$ tmux send-keys` command
- Configurable key layout (see: [default-layout.jsonc](default-layout.jsonc))

Limitations
-----------

- Cannot use full tmux features (such as pop-up windows)
- Cannot always display the cursor in the pane where keys are being sent
