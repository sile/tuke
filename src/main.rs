fn main() -> noargs::Result<()> {
    let app = tuke::app::App::new()?;
    app.run()?;
    Ok(())
}
