use std::env;

use crate::capture::{
    CaptureTarget, start_recording_detached, stop_recording_detached, take_screenshot,
};
use crate::ui::run_cli_recording_hud;

pub fn handle_cli_if_requested() -> Result<(), i32> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        return Ok(());
    }

    let result = match parse_cli_command(&args) {
        Ok(command) => run_cli_command(command),
        Err(message) => {
            eprintln!("{message}\n\n{}", cli_usage());
            Err(2)
        }
    };

    match result {
        Ok(()) => Err(0),
        Err(code) => Err(code),
    }
}

fn run_cli_command(command: CliCommand) -> Result<(), i32> {
    match command {
        CliCommand::Screenshot { target } => match take_screenshot(target) {
            Ok(path) => {
                println!("截图已保存: {}", path.display());
                Ok(())
            }
            Err(err) => {
                eprintln!("截图失败: {err}");
                Err(1)
            }
        },
        CliCommand::RecordStart { target, audio } => {
            match start_recording_detached(target, audio) {
                Ok(state) => {
                    println!(
                        "录屏已开始，输出文件: {}\n已显示右上角录制小窗，可在小窗中暂停/停止，或使用 `ncaptura record stop` 停止录屏。",
                        state.output_path.display()
                    );
                    run_cli_recording_hud(state);
                    Ok(())
                }
                Err(err) => {
                    eprintln!("开始录屏失败: {err}");
                    Err(1)
                }
            }
        }
        CliCommand::RecordStop => match stop_recording_detached() {
            Ok(path) => {
                println!("录屏已停止，文件保存为: {}", path.display());
                Ok(())
            }
            Err(err) => {
                eprintln!("停止录屏失败: {err}");
                Err(1)
            }
        },
        CliCommand::Help => {
            println!("{}", cli_usage());
            Ok(())
        }
    }
}

fn parse_cli_command(args: &[String]) -> Result<CliCommand, String> {
    if args[0] == "help" || args[0] == "--help" || args[0] == "-h" {
        return Ok(CliCommand::Help);
    }

    if args[0] == "screenshot" {
        if args.len() != 2 {
            return Err("screenshot 命令格式错误".to_string());
        }

        let target = parse_target(&args[1])?;
        return Ok(CliCommand::Screenshot { target });
    }

    if args[0] == "record" {
        if args.len() >= 2 && args[1] == "start" {
            if args.len() < 3 || args.len() > 4 {
                return Err("record start 命令格式错误".to_string());
            }

            let target = parse_target(&args[2])?;
            let audio = if args.len() == 4 {
                if args[3] == "--audio" {
                    true
                } else {
                    return Err("record start 仅支持 --audio 参数".to_string());
                }
            } else {
                false
            };

            return Ok(CliCommand::RecordStart { target, audio });
        }

        if args.len() == 2 && args[1] == "stop" {
            return Ok(CliCommand::RecordStop);
        }

        return Err("record 命令格式错误".to_string());
    }

    Err("未知命令".to_string())
}

fn parse_target(input: &str) -> Result<CaptureTarget, String> {
    match input {
        "region" => Ok(CaptureTarget::Region),
        "fullscreen" => Ok(CaptureTarget::Fullscreen),
        _ => Err(format!("不支持的目标类型: {input}")),
    }
}

fn cli_usage() -> &'static str {
    "NCaptura CLI

用法:
  ncaptura                      启动图形界面
  ncaptura screenshot region
  ncaptura screenshot fullscreen
  ncaptura record start region [--audio]
  ncaptura record start fullscreen [--audio]
  ncaptura record stop
  ncaptura help

niri 快捷键示例:
  Mod+Shift+S    { spawn \"ncaptura\" \"screenshot\" \"region\"; }
  Mod+Shift+F    { spawn \"ncaptura\" \"screenshot\" \"fullscreen\"; }
  Mod+Shift+R    { spawn \"ncaptura\" \"record\" \"start\" \"region\"; }
  Mod+Shift+A    { spawn \"ncaptura\" \"record\" \"start\" \"region\" \"--audio\"; }
  Mod+Shift+E    { spawn \"ncaptura\" \"record\" \"stop\"; }"
}

enum CliCommand {
    Screenshot { target: CaptureTarget },
    RecordStart { target: CaptureTarget, audio: bool },
    RecordStop,
    Help,
}
