use std::path::PathBuf;
use std::time::Duration;

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

    let options = tuke::app::AppOptions {
        cursor_refresh_interval: noargs::opt("cursor-refresh-interval")
            .ty("SECONDS")
            .env("TUKE_CURSOR_REFRESH_INTERVAL")
            .doc("Interval to refresh cursor visibility in the active pane")
            .default("1.0")
            .take(&mut args)
            .then(|a| a.value().parse().map(Duration::from_secs_f64))?,
    };

    if let Some(help) = args.finish()? {
        print!("{help}");
        return Ok(());
    }

    let layout = layout_file_path
        .map(|path| tuke::layout::Layout::load_from_file(path))
        .transpose()?
        .unwrap_or_default();
    let app = tuke::app::App::new(layout, options)?;
    app.run()?;
    Ok(())
}
