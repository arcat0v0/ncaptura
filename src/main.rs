mod app;
mod capture;
mod cli;
mod config;
mod ocr;
mod platform;
mod text;
mod translate;
mod ui;

fn main() {
    if let Err(code) = cli::handle_cli_if_requested() {
        std::process::exit(code);
    }

    app::run();
}
