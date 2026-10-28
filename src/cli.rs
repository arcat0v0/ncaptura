use std::env;
use std::fs;
use std::io::{Read, stdin};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};

use crate::capture::{
    CaptureTarget, start_recording_detached, stop_recording_detached, take_screenshot,
};
use crate::platform::clipboard::primary_selection_text;
use crate::ui::{
    run_cli_recording_hud, run_ocr_window, run_translate_input_window, run_translate_region_window,
    run_translate_selection_window,
};

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
        CliCommand::OcrSetup => match crate::ocr::setup_ocr_environment() {
            Ok(()) => Ok(()),
            Err(err) => {
                eprintln!("OCR 环境初始化失败: {err:#}");
                Err(1)
            }
        },
        CliCommand::Ocr { image } => {
            let image = match image {
                None => match take_screenshot(CaptureTarget::Region) {
                    Ok(path) => path,
                    Err(err) => {
                        eprintln!("截图失败: {err}");
                        return Err(1);
                    }
                },
                Some(source) => match materialize_ocr_image(source) {
                    Ok(path) => path,
                    Err(err) => {
                        eprintln!("读取图片失败: {err:#}");
                        return Err(1);
                    }
                },
            };

            run_ocr_window(image);
            Ok(())
        }
        CliCommand::TranslateRegion => match take_screenshot(CaptureTarget::Region) {
            Ok(path) => {
                run_translate_region_window(path);
                Ok(())
            }
            Err(err) => {
                eprintln!("截图失败: {err}");
                Err(1)
            }
        },
        CliCommand::TranslateSelection => match primary_selection_text() {
            Ok(text) => {
                run_translate_selection_window(text);
                Ok(())
            }
            Err(err) => {
                eprintln!("{err}");
                Err(1)
            }
        },
        CliCommand::TranslateInput => {
            run_translate_input_window();
            Ok(())
        }
        CliCommand::Version => {
            println!("ncaptura {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
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
    if matches!(args[0].as_str(), "version" | "--version" | "-v" | "-V") {
        return Ok(CliCommand::Version);
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

            let target = parse_record_target(&args[2])?;
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

    if args[0] == "ocr" {
        if args.len() == 1 {
            return Ok(CliCommand::Ocr { image: None });
        }
        if args.len() == 2 && args[1] == "setup" {
            return Ok(CliCommand::OcrSetup);
        }
        if args.len() == 2 {
            return Ok(CliCommand::Ocr {
                image: Some(parse_ocr_image_source(&args[1])?),
            });
        }

        return Err("ocr 命令格式错误".to_string());
    }

    if args[0] == "translate" {
        if args.len() != 2 {
            return Err("translate 命令格式错误".to_string());
        }

        return match args[1].as_str() {
            "region" => Ok(CliCommand::TranslateRegion),
            "selection" => Ok(CliCommand::TranslateSelection),
            "input" => Ok(CliCommand::TranslateInput),
            other => Err(format!("不支持的翻译方式: {other}")),
        };
    }

    Err("未知命令".to_string())
}

fn parse_ocr_image_source(input: &str) -> Result<OcrImageSource, String> {
    if input == "-" {
        return Ok(OcrImageSource::Stdin);
    }

    Ok(OcrImageSource::Path(PathBuf::from(input)))
}

fn materialize_ocr_image(source: OcrImageSource) -> Result<PathBuf> {
    match source {
        OcrImageSource::Path(path) => {
            let path = crate::config::expand_tilde(&path);
            if !path.is_file() {
                bail!("图片文件不存在: {}", path.display());
            }
            Ok(path)
        }
        OcrImageSource::Stdin => {
            let mut bytes = Vec::new();
            stdin()
                .read_to_end(&mut bytes)
                .context("读取标准输入失败")?;
            store_image_bytes(bytes)
        }
    }
}

fn store_image_bytes(bytes: Vec<u8>) -> Result<PathBuf> {
    if bytes.is_empty() {
        bail!("标准输入中没有图片数据");
    }

    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let path = env::temp_dir().join(format!("ncaptura-ocr-{timestamp}.png"));
    fs::write(&path, bytes).with_context(|| format!("写入临时图片失败: {}", path.display()))?;
    Ok(path)
}

fn parse_target(input: &str) -> Result<CaptureTarget, String> {
    match input {
        "region" => Ok(CaptureTarget::Region),
        "fullscreen" => Ok(CaptureTarget::Fullscreen),
        "window" => Ok(CaptureTarget::Window),
        _ => Err(format!("不支持的目标类型: {input}")),
    }
}

fn parse_record_target(input: &str) -> Result<CaptureTarget, String> {
    match input {
        "region" => Ok(CaptureTarget::Region),
        "fullscreen" => Ok(CaptureTarget::Fullscreen),
        "window" => Ok(CaptureTarget::Window),
        _ => Err(format!("不支持的录屏目标类型: {input}")),
    }
}

fn cli_usage() -> &'static str {
    "NCaptura CLI

用法:
  ncaptura                      启动图形界面
  ncaptura screenshot region
  ncaptura screenshot fullscreen
  ncaptura screenshot window
  ncaptura record start region [--audio]
  ncaptura record start fullscreen [--audio]
  ncaptura record start window [--audio]
  ncaptura record stop
  ncaptura ocr
  ncaptura ocr <图片路径>
  ncaptura ocr -
  ncaptura ocr setup
  ncaptura translate region
  ncaptura translate selection
  ncaptura translate input
  ncaptura version
  ncaptura help

niri 快捷键示例:
  Mod+Shift+S    { spawn \"ncaptura\" \"screenshot\" \"region\"; }
  Mod+Shift+F    { spawn \"ncaptura\" \"screenshot\" \"fullscreen\"; }
  Mod+Shift+R    { spawn \"ncaptura\" \"record\" \"start\" \"region\"; }
  Mod+Shift+A    { spawn \"ncaptura\" \"record\" \"start\" \"region\" \"--audio\"; }
  Mod+Shift+E    { spawn \"ncaptura\" \"record\" \"stop\"; }
  Mod+Shift+O    { spawn \"ncaptura\" \"ocr\"; }
  Mod+Shift+T    { spawn \"ncaptura\" \"translate\" \"region\"; }
  Mod+Shift+Y    { spawn \"ncaptura\" \"translate\" \"selection\"; }
  Mod+Shift+I    { spawn \"ncaptura\" \"translate\" \"input\"; }"
}

#[derive(Debug)]
enum OcrImageSource {
    Stdin,
    Path(PathBuf),
}

#[derive(Debug)]
enum CliCommand {
    Screenshot { target: CaptureTarget },
    RecordStart { target: CaptureTarget, audio: bool },
    RecordStop,
    Ocr { image: Option<OcrImageSource> },
    OcrSetup,
    TranslateRegion,
    TranslateSelection,
    TranslateInput,
    Version,
    Help,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_version_spellings() {
        for arg in ["version", "--version", "-v", "-V"] {
            let args = vec![arg.to_string()];
            assert!(matches!(parse_cli_command(&args), Ok(CliCommand::Version)));
        }
    }

    #[test]
    fn parses_window_recording_target() {
        let args = vec![
            "record".to_string(),
            "start".to_string(),
            "window".to_string(),
        ];
        assert!(matches!(
            parse_cli_command(&args),
            Ok(CliCommand::RecordStart {
                target: CaptureTarget::Window,
                audio: false,
            })
        ));
    }

    #[test]
    fn parses_screenshot_window_target() {
        let args = ["screenshot", "window"].map(String::from);
        assert!(matches!(
            parse_cli_command(&args),
            Ok(CliCommand::Screenshot {
                target: CaptureTarget::Window
            })
        ));
    }

    #[test]
    fn parses_bare_ocr_as_region_capture() {
        let args = ["ocr"].map(String::from);
        assert!(matches!(
            parse_cli_command(&args),
            Ok(CliCommand::Ocr { image: None })
        ));
    }

    #[test]
    fn parses_ocr_image_path() {
        let args = ["ocr", "~/shot.png"].map(String::from);
        match parse_cli_command(&args) {
            Ok(CliCommand::Ocr {
                image: Some(OcrImageSource::Path(path)),
            }) => assert_eq!(path, PathBuf::from("~/shot.png")),
            other => panic!("意外的解析结果: {other:?}"),
        }
    }

    #[test]
    fn parses_ocr_stdin_marker() {
        let args = ["ocr", "-"].map(String::from);
        assert!(matches!(
            parse_cli_command(&args),
            Ok(CliCommand::Ocr {
                image: Some(OcrImageSource::Stdin)
            })
        ));
    }

    #[test]
    fn rejects_ocr_with_extra_arguments() {
        let args = ["ocr", "a.png", "b.png"].map(String::from);
        assert!(parse_cli_command(&args).is_err());
    }

    #[test]
    fn materialize_rejects_missing_image_path() {
        let err = materialize_ocr_image(OcrImageSource::Path(PathBuf::from(
            "/nonexistent/ncaptura-missing.png",
        )))
        .unwrap_err();
        assert!(err.to_string().contains("图片文件不存在"));
    }

    #[test]
    fn materialize_keeps_existing_image_path() {
        let dir = env::temp_dir().join(format!("ncaptura-cli-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("shot.png");
        fs::write(&file, b"png").unwrap();

        let resolved = materialize_ocr_image(OcrImageSource::Path(file.clone())).unwrap();
        assert_eq!(resolved, file);

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn store_rejects_empty_image_bytes() {
        assert!(store_image_bytes(Vec::new()).is_err());
    }

    #[test]
    fn store_writes_image_bytes_to_temp_file() {
        let path = store_image_bytes(vec![1, 2, 3]).unwrap();
        assert_eq!(fs::read(&path).unwrap(), vec![1, 2, 3]);
        fs::remove_file(&path).unwrap();
    }

    #[test]
    fn parses_window_recording_with_audio() {
        let args = ["record", "start", "window", "--audio"].map(String::from);
        assert!(matches!(
            parse_cli_command(&args),
            Ok(CliCommand::RecordStart {
                target: CaptureTarget::Window,
                audio: true,
            })
        ));
    }

    #[test]
    fn rejects_invalid_window_recording_arguments() {
        for args in [
            vec!["record", "start", "window", "--invalid"],
            vec!["record", "start", "window", "--audio", "--audio"],
            vec!["record", "start", "unknown"],
        ] {
            let args: Vec<_> = args.into_iter().map(String::from).collect();
            assert!(parse_cli_command(&args).is_err());
        }
    }
}
