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

    let layout_file_path: Option<PathBuf> = noargs::opt("layout-file")
        .short('c')
        .ty("PATH")
        .doc("Path of layouturation JSONC file")
        .take(&mut args)
        .present_and_then(|a| a.value().parse())?;

    if let Some(help) = args.finish()? {
        print!("{help}");
        return Ok(());
    }

    let layout = layout_file_path
        .map(|path| tuke::config::Config::load_from_file(path))
        .transpose()?
        .unwrap_or_default();
    let app = tuke::app::App::new(layout)?;
    app.run()?;
    Ok(())
}
