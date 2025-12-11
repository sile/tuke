use std::path::PathBuf;

fn main() -> noargs::Result<()> {
    let mut args = noargs::raw_args();

    args.metadata_mut().app_name = env!("CARGO_PKG_NAME");
    args.metadata_mut().app_description = env!("CARGO_PKG_DESCRIPTION");

    if noargs::VERSION_FLAG.take(&mut args).is_present() {
        println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    noargs::HELP_FLAG.take_help(&mut args);

    let config_file_path: Option<PathBuf> = noargs::opt("config-file")
        .short('c')
        .ty("PATH")
        .env("TUKE_CONFIG_FILE")
        .doc("Path of configuration JSONC file")
        .take(&mut args)
        .present_and_then(|a| a.value().parse())?;

    if let Some(help) = args.finish()? {
        print!("{help}");
        return Ok(());
    }

    let app = tuke::app::App::new()?;
    app.run()?;
    Ok(())
}
