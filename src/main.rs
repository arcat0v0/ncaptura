mod app;
mod capture;
mod cli;
mod ui;

fn main() {
    if let Err(code) = cli::handle_cli_if_requested() {
        std::process::exit(code);
    }

    app::run();
}
