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

    /// Runs the poll loop until the child exits or the user quits.
    pub fn run(mut self) -> Result<()> {
        self.render()?;

        while !self.exit {
            // Drain everything the session can do without a new readiness
            // edge before blocking, so an edge-triggered poll cannot miss
            // work that produces no further edge. Rendering here, before the
            // wait, is what paints output the child produced while the loop
            // was last busy: without it the first screen stays blank until an
            // unrelated event (a key press) wakes the loop again.
            self.pump_session()?;
            if self.refresh_from_session() {
                self.render()?;
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
            // reported as the Escape key promptly; otherwise block until an
            // fd is ready.
            let timeout = if self.input.has_uncommitted_escape() {
                timeout_to_millis(ESCAPE_TIMEOUT)
            } else {
                -1
            };

            let n = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, timeout) };
            if n < 0 {
                let error = std::io::Error::last_os_error();
                if error.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(error.into());
            }

            let mut dirty = false;

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

            // The session may have produced output or become writable.
            self.pump_session()?;
            if self.refresh_from_session() {
                dirty = true;
            }

            if dirty {
                self.render()?;
            }
        }

        Ok(())
    }

    /// Records the child terminal's current revision and reports whether it
    /// changed since the last render.
    fn refresh_from_session(&mut self) -> bool {
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
        while self.session.needs_pump() {
            self.session.pump_io(termnix::PumpBudget::default())?;
        }
        // Reap the child if it has exited.
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
    /// Returns whether the screen needs repainting.
    fn handle_input(&mut self, input: tuinix::Input) -> Result<bool> {
        match input {
            tuinix::Input::Key(key) => self.dispatch(Event::Key {
                code: key.code,
                ctrl: key.ctrl,
            }),
            tuinix::Input::Mouse(mouse) => {
                if mouse.kind != tuinix::MouseInputKind::LeftRelease {
                    return Ok(false);
                }
                self.dispatch(Event::PointerRelease {
                    position: mouse.position,
                })
            }
            tuinix::Input::Unrecognized { .. } | tuinix::Input::Paste { .. } => Ok(false),
        }
    }

    /// Runs the core transition and carries out its actions.
    fn dispatch(&mut self, event: Event) -> Result<bool> {
        let actions = self.state.update(event);
        if trace_enabled() {
            eprintln!("[tuke] event={event:?} -> actions={actions:?}");
        }
        let mut dirty = false;
        for action in actions {
            match action {
                Action::SendKey(key) => {
                    if trace_enabled() {
                        let bytes = self.session.input_byte_len(termnix::Input::Key(key));
                        eprintln!("[tuke] send key={key:?} bytes={bytes}");
                    }
                    self.session.enqueue_input(termnix::Input::Key(key))?;
                    self.pump_session()?;
                }
                Action::SendBytes(bytes) => {
                    self.session.enqueue_input(termnix::Input::Raw(&bytes))?;
                    self.pump_session()?;
                }
                Action::ResizeSession(size) => {
                    if let Some(size) = geometry::to_termnix_size(size) {
                        self.session.resize(size)?;
                        self.pump_session()?;
                    }
                }
                Action::Redraw => dirty = true,
                Action::Quit => {
                    self.exit = true;
                    dirty = true;
                }
            }
        }
        Ok(dirty)
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
