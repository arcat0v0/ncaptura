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
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("__record-window") {
        let Some(output_path) = args.get(2) else {
            eprintln!("窗口录屏缺少输出文件");
            std::process::exit(2);
        };
        let with_audio = args.iter().any(|arg| arg == "--audio");
        let result = platform::recording::run_window_recording(output_path.into(), with_audio);
        if let Err(err) = result {
            eprintln!("窗口录屏失败: {err:#}");
            std::process::exit(1);
        }
        return;
    }

    if let Err(code) = cli::handle_cli_if_requested() {
        std::process::exit(code);
    }

    app::run();
}
