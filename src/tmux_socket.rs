use std::io::Write;
use std::os::unix::net::UnixStream;

use orfail::OrFail;

#[derive(Debug)]
pub struct TmuxSocket {
    stream: UnixStream,
}

impl TmuxSocket {
    pub fn new() -> orfail::Result<Self> {
        let tmux_socket = std::env::var("TMUX").or_fail()?;
        let socket_path = tmux_socket.split(',').next().or_fail()?;
        let stream = UnixStream::connect(socket_path).or_fail()?;
        Ok(Self { stream })
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
        writeln!(self.stream, "{}", cmd_string).or_fail()?;
        Ok(())
    }
}
