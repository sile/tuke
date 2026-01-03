use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

use orfail::OrFail;

/// Control mode client for tmux communication
///
/// Doc: https://github.com/tmux/tmux/wiki/Control-Mode
#[derive(Debug)]
pub struct TmuxClient {
    stdin: std::process::ChildStdin,
    reader: BufReader<std::process::ChildStdout>,
}

impl TmuxClient {
    pub fn new() -> orfail::Result<Self> {
        // Start tmux in control mode (-C) attached to the default session
        let mut child = Command::new("tmux")
            .arg("-C")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .or_fail()?;

        let stdin = child.stdin.take().or_fail()?;
        let stdout = child.stdout.take().or_fail()?;
        let reader = BufReader::new(stdout);

        Ok(Self { stdin, reader })
    }

    pub fn send_command(&mut self, command: &str, args: &[&str]) -> orfail::Result<()> {
        let mut cmd_string = format!("{}", command);
        for arg in args {
            cmd_string.push(' ');
            if arg.contains(' ') || arg.contains(';') {
                cmd_string.push('"');
                cmd_string.push_str(&arg.replace('"', "\\\""));
                cmd_string.push('"');
            } else {
                cmd_string.push_str(arg);
            }
        }

        // Send command to control mode client
        writeln!(self.stdin, "{}", cmd_string).or_fail()?;
        self.stdin.flush().or_fail()?;

        // Read response until %end or %error marker
        let mut response = String::new();
        loop {
            response.clear();
            self.reader.read_line(&mut response).or_fail()?;

            // Control mode responses are wrapped in %begin/%end or %begin/%error
            if response.starts_with("%end") {
                return Ok(());
            }
            if response.starts_with("%error") {
                return Err(orfail::Failure::new("tmux command failed"));
            }
        }
    }
}
