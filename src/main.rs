mod app;

use std::path::PathBuf;
use std::process::Command;

fn main() -> noargs::Result<()> {
    let mut args = noargs::raw_args();

    args.metadata_mut().app_name = env!("CARGO_PKG_NAME");
    args.metadata_mut().app_description = env!("CARGO_PKG_DESCRIPTION");

    if noargs::VERSION_FLAG.take(&mut args).is_present() {
        println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    noargs::HELP_FLAG.take_help(&mut args);

    let layout_file_path: Option<PathBuf> = noargs::opt("layout-file")
        .short('l')
        .ty("PATH")
        .env("TUKE_LAYOUT_FILE")
        .doc("Path of layout JSONC file")
        .take(&mut args)
        .present_and_then(|a| a.value().parse())?;

    let command_line: Option<String> = noargs::opt("command")
        .short('c')
        .ty("COMMAND")
        .doc("Command to run in the session (defaults to $SHELL -l)")
        .take(&mut args)
        .present_and_then(|a| a.value().parse())?;

    let mut command = match command_line {
        Some(line) => {
            let mut command = Command::new("sh");
            command.arg("-c").arg(line);
            command
        }
        None => {
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_owned());
            let mut command = Command::new(shell);
            command.arg("-l");
            command
        }
    };

    if let Some(help) = args.finish()? {
        print!("{help}");
        return Ok(());
    }

    let layout = layout_file_path
        .map(tuke::layout::Layout::load_from_file)
        .transpose()?
        .unwrap_or_default();
    let app = app::App::new(layout, &mut command)?;
    app.run()?;
    Ok(())
}
