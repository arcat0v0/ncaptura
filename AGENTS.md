# Repository Guidelines

## Project Structure & Module Organization
This project is a Rust desktop capture tool (`ncaptura`) built with GTK4 + Libadwaita.

- `src/main.rs`: minimal entry point (`CLI -> GUI` bootstrap).
- `src/cli.rs`: command parsing and CLI execution (`screenshot`, `record start/stop`).
- `src/app.rs`: GUI workflow orchestration (mode handling, delayed capture, save flow).
- `src/capture/`: capture domain logic, split by responsibility:
  - `screenshot.rs`, `recording.rs`, `windows.rs`, `state.rs`, `output.rs`, `command_utils.rs`.
- `src/ui/`: presentation layer components:
  - `interactive_dialog.rs`, `recording_hud.rs`, `window_picker.rs`, `save_dialog.rs`.
- `design/`: local design artifacts and mockups (non-runtime assets).

## Build, Test, and Development Commands
- `cargo check`: fast compile validation without producing a release binary.
- `cargo build`: debug build for local verification.
- `cargo run`: launch app (GUI mode).
- `cargo run -- screenshot region`: run CLI path quickly.
- `cargo test`: run all tests (currently mostly compile-level verification).
- `cargo fmt`: format code with rustfmt.

Run `cargo fmt && cargo check && cargo test` before opening a PR.

## Coding Style & Naming Conventions
- Follow Rust 2024 idioms and default `rustfmt` output (4-space indentation, trailing commas where formatted).
- Use `snake_case` for functions/modules/files, `PascalCase` for types/enums, `SCREAMING_SNAKE_CASE` for constants.
- Keep modules focused: UI in `src/ui`, system/process logic in `src/capture`, orchestration in `src/app.rs`.
- Prefer `anyhow::Result` + contextual errors for external command failures.

## Testing Guidelines
- Add unit tests in the same file using `#[cfg(test)] mod tests`.
- Prioritize deterministic logic (CLI parsing, path/state helpers) over compositor-dependent flows.
- Name tests by behavior, e.g., `parse_record_start_with_audio`.
- For command-dependent behavior (`grim`, `wf-recorder`, `niri`), isolate parsing/decision logic for testability.

## Commit & Pull Request Guidelines
Recent history uses short, imperative summaries, often with emoji prefixes (e.g., `✨ add selectable window screenshot flow`).

- Commit format: `<emoji optional> <imperative summary>`.
- Keep commits scoped to one concern (CLI, capture, UI, refactor).
- PRs should include:
  - what changed and why,
  - manual verification steps,
  - screenshots/GIFs for UI changes,
  - linked issue/task when available.
