//! The I/O edge: poll loop, PTY session, raw terminal, and frame diffing.
//!
//! This module is compiled into the binary only (`mod app;` in `main.rs`); the
//! library stays Sans I/O. It owns the [`termnix::Session`] driving one child
//! process and translates [`State::update`](tuke::state::State::update)'s
//! actions into real effects.

use std::io::{Read, Write};
use std::process::Command;
use std::time::Duration;

use tuke::action::Action;
use tuke::error::Result;
use tuke::event::Event;
use tuke::geometry;
use tuke::layout::Layout;
use tuke::render;
use tuke::state::State;

/// How long to wait for the rest of an escape sequence before treating a lone
/// `ESC` byte as the Escape key.
const ESCAPE_TIMEOUT: Duration = Duration::from_millis(50);

/// The edge owns everything with a file descriptor.
pub struct App {
    driver: tuinix::TerminalDriver,
    input: tuinix::InputDecoder,
    prev_frame: Option<tuinix::Frame>,
    session: termnix::Session,
    state: State,
    terminal_size: tuinix::Size,
    /// The terminal revision at the last render, used to detect output.
    last_revision: u64,
    /// Set once the child has been reaped; the loop ends after the turn that
    /// notices it, so the child's last output is painted first.
    exit: bool,
}

impl App {
    /// Spawns the child command and takes over the terminal.
    pub fn new(layout: Layout, command: &mut Command) -> Result<Self> {
        let mut driver = tuinix::TerminalDriver::new()?;
        driver.enable_mouse_reporting()?;

        let terminal_size = driver.size();
        let state = State::new(layout, terminal_size);

        let session_size = geometry::to_termnix_size(state.grid_size())
            .ok_or_else(|| tuke::Error::message("terminal too small to fit the keyboard layout"))?;
        let session = termnix::Session::new(command, session_size)?;

        Ok(Self {
            driver,
            input: tuinix::InputDecoder::new(),
            prev_frame: None,
            session,
            state,
            terminal_size,
            last_revision: 0,
            exit: false,
        })
    }

    /// Runs the poll loop until the child exits.
    ///
    /// The child's exit is the only thing that ends the loop: tuke reserves
    /// no key of its own, so there is no quit key to watch. Each turn is the
    /// same three steps, in this order: flush every effect the core has asked
    /// for and read back the child's response, repaint if anything visible
    /// moved, then wait for something to happen next. Doing the flush and the
    /// repaint *before* the wait is what keeps the first screen from staying
    /// blank until an unrelated event wakes the wait.
    pub fn run(mut self) -> Result<()> {
        // Whether the screen is out of date. It is driven from both sources of
        // visible change: the child's terminal revision, for the grid, and the
        // core's own redraw requests, for the keyboard's highlights.
        let mut dirty = true;

        while !self.exit {
            // Flush the outstanding effects and read back whatever the child
            // wrote in response, so the repaint below sees it.
            self.pump_session()?;
            dirty |= self.child_moved();
            if dirty {
                self.render()?;
                dirty = false;
            }
            if self.exit {
                break;
            }

            let Some(session_fd) = self.session.fd() else {
                break;
            };

            let mut fds = [
                libc::pollfd {
                    fd: self.driver.resize_signal_fd(),
                    events: libc::POLLIN,
                    revents: 0,
                },
                libc::pollfd {
                    fd: self.driver.input_fd(),
                    events: libc::POLLIN,
                    revents: 0,
                },
                libc::pollfd {
                    fd: session_fd,
                    events: self.session_interests(),
                    revents: 0,
                },
            ];

            // Wait only briefly while a lone `ESC` byte is held, so it is
            // reported as the Escape key promptly; otherwise block until a
            // fd is ready.
            let timeout = if self.input.has_uncommitted_escape() {
                timeout_to_millis(ESCAPE_TIMEOUT)
            } else {
                -1
            };

            if trace_enabled() {
                eprintln!(
                    "[tuke] poll waiting interests={:#x} timeout={timeout}",
                    fds[2].events
                );
            }

            #[expect(
                unsafe_code,
                reason = "libc::poll is the only way to wait on the resize, input, and PTY fds at once"
            )]
            let n = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, timeout) };
            if n < 0 {
                let error = std::io::Error::last_os_error();
                if error.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(error.into());
            }

            if trace_enabled() {
                eprintln!(
                    "[tuke] poll woke n={n} resize={:#x} stdin={:#x} session={:#x}",
                    fds[0].revents, fds[1].revents, fds[2].revents
                );
            }

            if fds[0].revents & libc::POLLIN != 0 {
                self.driver.handle_resize_signal()?;
                let size = self.driver.size();
                if size != self.terminal_size {
                    self.terminal_size = size;
                    dirty |= self.dispatch(Event::Resize { size })?;
                }
            }

            if fds[1].revents & libc::POLLIN != 0 {
                self.read_stdin()?;
            }

            if n == 0 {
                // The wait elapsed with a lone `ESC` still held.
                self.input.commit_escape();
            }

            while let Some(input) = self.input.next() {
                dirty |= self.handle_input(input)?;
            }

            // Write anything the core just asked for now, rather than leaving
            // it queued for the top of the next turn. A key that is still in
            // the session's write buffer has not reached the child, and the
            // wait below must not block on a child that was never given the
            // input it is waiting for.
            //
            // This is unconditional for the same reason the top of the loop
            // is: `needs_pump()` is false after a `WouldBlock`, and only
            // `pump_io` clears that, so gating on it here could leave a queued
            // key unwritten across the wait.
            self.pump_session()?;
            dirty |= self.child_moved();
        }

        Ok(())
    }

    /// Reports whether the child terminal has moved since the last repaint,
    /// updating the recorded revision when it has.
    ///
    /// This covers the grid; the keyboard is covered by the core's own redraw
    /// requests, which [`dispatch()`](Self::dispatch) reports.
    fn child_moved(&mut self) -> bool {
        let revision = self.session.terminal_state().revision();
        if revision == self.last_revision {
            return false;
        }
        self.last_revision = revision;
        true
    }

    fn session_interests(&self) -> libc::c_short {
        let interests = self.session.interests();
        let mut events = 0;
        if interests.readable {
            events |= libc::POLLIN;
        }
        if interests.writable {
            events |= libc::POLLOUT;
        }
        events
    }

    fn pump_session(&mut self) -> Result<()> {
        // The first pump is unconditional. `needs_pump` is false right after a
        // `read` hit `WouldBlock`, so gating on it would skip the read
        // entirely and leave the fd readable: `poll` would then return
        // immediately, forever, without the child's output ever being read.
        // Pumping once clears that flag and picks up what arrived; the drain
        // below then finishes any work the budget left behind.
        let mut pumps = 0u32;
        loop {
            self.session.pump_io(termnix::PumpBudget::default())?;
            pumps += 1;
            if !self.session.needs_pump() {
                break;
            }
            if pumps > 1000 {
                eprintln!("[tuke] pump did not settle after {pumps} calls; breaking");
                break;
            }
        }
        if trace_enabled() {
            let c = self.session.counters();
            eprintln!(
                "[tuke] pump calls={pumps} status={:?} revision={} read={} read_wb={} readsys={} interests={:?}",
                self.session.status(),
                self.session.terminal_state().revision(),
                c.pty_bytes_read,
                c.read_would_block,
                c.read_syscalls,
                self.session.interests(),
            );
        }
        // The child exiting is what ends tuke. The flag is set here rather
        // than mid-turn, so the output this pump just read is rendered before
        // the loop checks it.
        if self.session.try_wait()?.is_some() {
            self.exit = true;
        }
        Ok(())
    }

    fn read_stdin(&mut self) -> Result<()> {
        let mut bytes = [0u8; 1024];
        loop {
            match self.driver.read(&mut bytes) {
                Ok(0) => break,
                Ok(n) => self.input.feed(&bytes[..n]),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e.into()),
            }
        }
        Ok(())
    }

    /// Translates one host input event into a core event and dispatches it.
    ///
    /// Returns whether the core asked for a repaint of its keyboard.
    fn handle_input(&mut self, input: tuinix::Input) -> Result<bool> {
        match input {
            tuinix::Input::Key(key) => self.dispatch(Event::Key {
                code: key.code,
                ctrl: key.ctrl,
                alt: key.alt,
            }),

            tuinix::Input::Mouse(mouse) => {
                if mouse.kind != tuinix::MouseInputKind::LeftRelease {
                    return Ok(false);
                }
                self.dispatch(Event::PointerRelease {
                    position: mouse.position,
                })
            }
            tuinix::Input::Paste { bytes } => self.dispatch(Event::Paste { bytes }),
            tuinix::Input::Unrecognized { .. } => Ok(false),
        }
    }

    /// Runs the core transition and carries out its actions, returning whether
    /// the core asked for a repaint.
    ///
    /// The child's output this dispatch provokes is not reported here: the
    /// caller picks it up from the child terminal's revision, which it checks
    /// alongside this signal.
    fn dispatch(&mut self, event: Event) -> Result<bool> {
        // The event is kept for the trace, so the transition is given a copy.
        // A paste body can be large, but this only happens under `TUKE_TRACE`.
        let traced = trace_enabled().then(|| event.clone());
        let actions = self.state.update(event);
        if let Some(traced) = traced {
            eprintln!("[tuke] event={traced:?} -> actions={actions:?}");
        }
        let mut redraw = false;
        for action in actions {
            match action {
                Action::SendKey(key) => {
                    if trace_enabled() {
                        let bytes = self.session.input_byte_len(termnix::Input::Key(key));
                        eprintln!("[tuke] send key={key:?} bytes={bytes}");
                    }
                    self.session.enqueue_input(termnix::Input::Key(key))?;
                }
                Action::SendBytes(bytes) => {
                    self.session.enqueue_input(termnix::Input::Raw(&bytes))?;
                }
                Action::SendPaste(text) => {
                    self.session.enqueue_input(termnix::Input::Paste(&text))?;
                }
                Action::ResizeSession(size) => {
                    if let Some(size) = geometry::to_termnix_size(size) {
                        self.session.resize(size)?;
                    }
                }
                Action::Redraw => redraw = true,
            }
        }
        Ok(redraw)
    }

    fn render(&mut self) -> Result<()> {
        let terminal = self.session.terminal_state();
        let frame = render::frame(&self.state, terminal, self.terminal_size);
        let cursor = render::cursor(terminal, self.terminal_size);
        let out = frame.render(self.prev_frame.as_ref(), cursor);
        self.driver.write_all(&out)?;
        self.driver.flush()?;
        self.prev_frame = Some(frame);
        Ok(())
    }
}

/// Whether `TUKE_TRACE` is set to a non-empty value, enabling the debug trace
/// on stderr of every dispatched event and sent key.
fn trace_enabled() -> bool {
    std::env::var_os("TUKE_TRACE").is_some_and(|value| !value.is_empty())
}

/// Converts a timeout to the millisecond value `libc::poll` expects.
fn timeout_to_millis(timeout: Duration) -> libc::c_int {
    timeout.as_millis().min(libc::c_int::MAX as u128) as libc::c_int
}
