// Embeds assets/clearshot.ico into clearshot.exe (Explorer, shortcuts, Alt+Tab).
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("assets/clearshot.ico")
            .compile()
            .expect("failed to embed the icon");
    }
}
