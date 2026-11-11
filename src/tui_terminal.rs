use std::io::{IsTerminal, Stdout, Write};
use std::process::{Command, Stdio};

use crossterm::cursor::Show;
use crossterm::event::{
    self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, KeyboardEnhancementFlags, MouseButton,
    MouseEvent, MouseEventKind, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::tui::{TuiState, WELCOME_LOGO, sanitize_inline, welcome_logo_green_range};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TuiInput {
    Char(char),
    Enter,
    Backspace,
    Delete,
    Left,
    Right,
    Up,
    Down,
    PageUp,
    PageDown,
    Home,
    End,
    Tab,
    CtrlTab,
    CommandPalette,
    Escape,
    CtrlC,
    CtrlO,
    CtrlQ,
    Paste(String),
    PasteClipboard,
    RightClick { column: u16, row: u16 },
    MouseDown { column: u16, row: u16 },
    MouseDrag { column: u16, row: u16 },
    MouseUp { column: u16, row: u16 },
    MouseMove { column: u16, row: u16 },
    ScrollUp,
    ScrollDown,
    Resize,
}

pub fn interactive_terminal_available() -> bool {
    cfg!(unix) && std::io::stdin().is_terminal() && std::io::stdout().is_terminal()
}

pub fn terminal_dimensions() -> (usize, usize) {
    if let (Some(columns), Some(rows)) = (
        std::env::var("COLUMNS")
            .ok()
            .and_then(|value| value.parse::<usize>().ok()),
        std::env::var("LINES")
            .ok()
            .and_then(|value| value.parse::<usize>().ok()),
    ) {
        return (columns, rows);
    }
    crossterm::terminal::size()
        .map(|(columns, rows)| (usize::from(columns), usize::from(rows)))
        .unwrap_or((100, 32))
}

fn install_panic_hook() {
    static INSTALLED: std::sync::Once = std::sync::Once::new();
    INSTALLED.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let _ = disable_raw_mode();
            let mut stdout = std::io::stdout();
            let _ = execute!(
                stdout,
                DisableMouseCapture,
                DisableBracketedPaste,
                LeaveAlternateScreen,
                Show
            );
            let message = info.to_string();
            let path = crate::runtime_home::default_home().join("tui-crash.log");
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
            {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|duration| duration.as_millis())
                    .unwrap_or_default();
                let _ = writeln!(file, "{now} tui panic: {message}");
            }
            eprintln!("1Agent TUI panic; crash log: {}\n{message}", path.display());
            previous(info);
        }));
    });
}

pub fn spawn_input_reader() -> tokio::sync::mpsc::UnboundedReceiver<TuiInput> {
    let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
    std::thread::spawn(move || {
        let mut failures = 0u8;
        loop {
            match event::read() {
                Ok(event) => {
                    failures = 0;
                    let Some(input) = map_event(event) else {
                        continue;
                    };
                    if sender.send(input).is_err() {
                        break;
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error) => {
                    failures = failures.saturating_add(1);
                    if failures >= 3 {
                        let path = crate::runtime_home::default_home().join("tui-crash.log");
                        if let Ok(mut file) = std::fs::OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(&path)
                        {
                            let _ = writeln!(file, "input reader stopped: {error}");
                        }
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
            }
        }
    });
    receiver
}

fn map_event(event: Event) -> Option<TuiInput> {
    match event {
        Event::Key(key) => map_key(key),
        Event::Mouse(mouse) => map_mouse(mouse),
        Event::Paste(text) => Some(TuiInput::Paste(text)),
        Event::Resize(_, _) => Some(TuiInput::Resize),
        Event::FocusGained | Event::FocusLost => None,
    }
}

fn map_key(key: KeyEvent) -> Option<TuiInput> {
    if matches!(key.kind, KeyEventKind::Release) {
        return None;
    }
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Tab => return Some(TuiInput::CtrlTab),
            KeyCode::Char('c') | KeyCode::Char('C') => return Some(TuiInput::CtrlC),
            KeyCode::Char('o') | KeyCode::Char('O') => return Some(TuiInput::CtrlO),
            KeyCode::Char('q') | KeyCode::Char('Q') => return Some(TuiInput::CtrlQ),
            KeyCode::Char('t') | KeyCode::Char('T') => return Some(TuiInput::CtrlTab),
            KeyCode::Char('k') | KeyCode::Char('K') => return Some(TuiInput::CommandPalette),
            KeyCode::Char('v') | KeyCode::Char('V') => {
                return Some(TuiInput::PasteClipboard);
            }
            _ => {}
        }
    }
    let input = match key.code {
        KeyCode::Char(character)
            if !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER) =>
        {
            TuiInput::Char(character)
        }
        KeyCode::Enter => TuiInput::Enter,
        KeyCode::Backspace => TuiInput::Backspace,
        KeyCode::Delete => TuiInput::Delete,
        KeyCode::Left => TuiInput::Left,
        KeyCode::Right => TuiInput::Right,
        KeyCode::Up => TuiInput::Up,
        KeyCode::Down => TuiInput::Down,
        KeyCode::PageUp => TuiInput::PageUp,
        KeyCode::PageDown => TuiInput::PageDown,
        KeyCode::Home => TuiInput::Home,
        KeyCode::End => TuiInput::End,
        KeyCode::Tab => TuiInput::Tab,
        // Shift+Tab is the portable fallback: many terminals cannot report
        // Ctrl+Tab separately unless they speak a keyboard-enhancement protocol.
        KeyCode::BackTab => TuiInput::CtrlTab,
        KeyCode::Esc => TuiInput::Escape,
        _ => return None,
    };
    Some(input)
}

fn map_mouse(mouse: MouseEvent) -> Option<TuiInput> {
    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => Some(TuiInput::MouseDown {
            column: mouse.column,
            row: mouse.row,
        }),
        MouseEventKind::Drag(MouseButton::Left) => Some(TuiInput::MouseDrag {
            column: mouse.column,
            row: mouse.row,
        }),
        MouseEventKind::Up(MouseButton::Left) => Some(TuiInput::MouseUp {
            column: mouse.column,
            row: mouse.row,
        }),
        MouseEventKind::Moved => Some(TuiInput::MouseMove {
            column: mouse.column,
            row: mouse.row,
        }),
        MouseEventKind::Down(MouseButton::Right) => Some(TuiInput::RightClick {
            column: mouse.column,
            row: mouse.row,
        }),
        MouseEventKind::ScrollUp => Some(TuiInput::ScrollUp),
        MouseEventKind::ScrollDown => Some(TuiInput::ScrollDown),
        _ => None,
    }
}

pub struct TerminalSession {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    active: bool,
}

impl TerminalSession {
    pub fn enter() -> std::io::Result<Self> {
        install_panic_hook();
        enable_raw_mode()?;
        let mut stdout = std::io::stdout();
        execute!(
            stdout,
            EnterAlternateScreen,
            PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES),
            EnableMouseCapture,
            EnableBracketedPaste
        )?;
        let backend = CrosstermBackend::new(stdout);
        // The alternate screen starts blank. Do not query cursor position here:
        // Workshop may launch the PTY before its GUI attaches, so a blocking
        // position read would make the Master-Agent session exit during startup.
        let terminal = Terminal::new(backend)?;
        Ok(Self {
            terminal,
            active: true,
        })
    }

    pub fn draw(&mut self, state: &mut TuiState) -> std::io::Result<()> {
        self.terminal.draw(|frame| state.draw(frame))?;
        Ok(())
    }

    fn restore(&mut self) {
        if !self.active {
            return;
        }
        let _ = disable_raw_mode();
        let _ = execute!(
            self.terminal.backend_mut(),
            PopKeyboardEnhancementFlags,
            DisableMouseCapture,
            DisableBracketedPaste,
            LeaveAlternateScreen
        );
        let _ = self.terminal.show_cursor();
        self.active = false;
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        self.restore();
    }
}

pub fn copy_to_clipboard(text: &str) -> std::io::Result<()> {
    if text.is_empty() {
        return Ok(());
    }
    for (program, arguments) in clipboard_copy_commands() {
        if write_to_command(program, arguments, text).is_ok() {
            return Ok(());
        }
    }
    write_terminal(&format!("\x1b]52;c;{}\x07", base64(text.as_bytes())))
}

pub fn read_clipboard() -> std::io::Result<String> {
    for (program, arguments) in clipboard_paste_commands() {
        let output = Command::new(program)
            .args(*arguments)
            .stdin(Stdio::null())
            .output();
        if let Ok(output) = output
            && output.status.success()
        {
            return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
        }
    }
    Err(std::io::Error::other("clipboard paste is unavailable"))
}

pub fn print_session_banner(title: &str, continue_command: &str) -> std::io::Result<()> {
    let use_color = std::env::var_os("NO_COLOR").is_none();
    let mut stdout = std::io::stdout().lock();
    write_session_banner(&mut stdout, title, continue_command, use_color)?;
    stdout.flush()
}

pub fn write_session_banner(
    writer: &mut impl Write,
    title: &str,
    continue_command: &str,
    use_color: bool,
) -> std::io::Result<()> {
    const WORD: &str = "\x1b[38;2;72;119;72m";
    const GREEN: &str = "\x1b[1;38;5;10m";
    const RESET: &str = "\x1b[0m";

    writeln!(writer)?;
    for (index, line) in WELCOME_LOGO.iter().enumerate() {
        if !use_color {
            writeln!(writer, "{line}")?;
            continue;
        }
        let green = welcome_logo_green_range(index);
        let green_start = char_byte_index(line, green.start);
        let green_end = char_byte_index(line, green.end);
        write!(writer, "{WORD}{}", &line[..green_start])?;
        if green_start < green_end {
            write!(writer, "{GREEN}{}{WORD}", &line[green_start..green_end])?;
        }
        writeln!(writer, "{}{RESET}", &line[green_end..])?;
    }
    writeln!(writer)?;
    writeln!(writer, "  会话      {}", sanitize_inline(title))?;
    writeln!(writer, "  继续      {}", sanitize_inline(continue_command))
}

fn char_byte_index(text: &str, char_index: usize) -> usize {
    text.char_indices()
        .nth(char_index)
        .map_or(text.len(), |(index, _)| index)
}

fn write_to_command(program: &str, arguments: &[&str], text: &str) -> std::io::Result<()> {
    let mut child = Command::new(program)
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(text.as_bytes())?;
    }
    let status = child.wait()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other("clipboard command failed"))
    }
}

#[cfg(target_os = "macos")]
fn clipboard_copy_commands() -> &'static [(&'static str, &'static [&'static str])] {
    &[("pbcopy", &[])]
}

#[cfg(target_os = "linux")]
fn clipboard_copy_commands() -> &'static [(&'static str, &'static [&'static str])] {
    &[
        ("wl-copy", &[]),
        ("xclip", &["-selection", "clipboard"]),
        ("xsel", &["--clipboard", "--input"]),
    ]
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn clipboard_copy_commands() -> &'static [(&'static str, &'static [&'static str])] {
    &[]
}

#[cfg(target_os = "macos")]
fn clipboard_paste_commands() -> &'static [(&'static str, &'static [&'static str])] {
    &[("pbpaste", &[])]
}

#[cfg(target_os = "linux")]
fn clipboard_paste_commands() -> &'static [(&'static str, &'static [&'static str])] {
    &[
        ("wl-paste", &["--no-newline"]),
        ("xclip", &["-selection", "clipboard", "-o"]),
        ("xsel", &["--clipboard", "--output"]),
    ]
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn clipboard_paste_commands() -> &'static [(&'static str, &'static [&'static str])] {
    &[]
}

fn write_terminal(output: &str) -> std::io::Result<()> {
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(output.as_bytes())?;
    stdout.flush()
}

fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        output.push(TABLE[usize::from(first >> 2)] as char);
        output.push(TABLE[usize::from(((first & 0x03) << 4) | (second >> 4))] as char);
        output.push(if chunk.len() > 1 {
            TABLE[usize::from(((second & 0x0f) << 2) | (third >> 6))] as char
        } else {
            '='
        });
        output.push(if chunk.len() > 2 {
            TABLE[usize::from(third & 0x3f)] as char
        } else {
            '='
        });
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_keys_and_control_shortcuts() {
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Char('你'), KeyModifiers::NONE)),
            Some(TuiInput::Char('你'))
        );
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            Some(TuiInput::CtrlC)
        );
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::CONTROL)),
            Some(TuiInput::PasteClipboard)
        );
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL)),
            Some(TuiInput::CtrlO)
        );
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)),
            Some(TuiInput::Tab)
        );
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::CONTROL)),
            Some(TuiInput::CtrlTab)
        );
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL)),
            Some(TuiInput::CommandPalette)
        );
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL)),
            Some(TuiInput::CtrlTab)
        );
        assert_eq!(
            map_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT)),
            Some(TuiInput::CtrlTab)
        );
    }

    #[test]
    fn maps_mouse_events_after_crossterm_parsing() {
        let mut mouse = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 12,
            row: 8,
            modifiers: KeyModifiers::NONE,
        };
        assert_eq!(
            map_mouse(mouse),
            Some(TuiInput::MouseDown { column: 12, row: 8 })
        );
        mouse.kind = MouseEventKind::Drag(MouseButton::Left);
        assert_eq!(
            map_mouse(mouse),
            Some(TuiInput::MouseDrag { column: 12, row: 8 })
        );
        mouse.kind = MouseEventKind::Down(MouseButton::Right);
        assert_eq!(
            map_mouse(mouse),
            Some(TuiInput::RightClick { column: 12, row: 8 })
        );
        mouse.kind = MouseEventKind::Moved;
        assert_eq!(
            map_mouse(mouse),
            Some(TuiInput::MouseMove { column: 12, row: 8 })
        );
    }

    #[test]
    fn base64_matches_rfc_examples() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"hello"), "aGVsbG8=");
    }

    #[test]
    fn session_banner_reuses_colored_welcome_logo() {
        let mut output = Vec::new();
        write_session_banner(
            &mut output,
            "Greeting",
            "1agent tui --session ses_example",
            true,
        )
        .unwrap();
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("\x1b[38;2;72;119;72m"));
        assert!(output.contains("\x1b[1;38;5;10m"));
        // The banner writes the logo one styled span at a time, so the plain text
        // is not contiguous; assert on the glyph the art is drawn with.
        let glyph = crate::tui::WELCOME_LOGO[0]
            .trim()
            .chars()
            .next()
            .expect("logo is not empty");
        assert!(
            output.contains(glyph),
            "banner should reuse the welcome logo"
        );
        assert!(output.contains("会话      Greeting"));
        assert!(output.contains("继续      1agent tui --session ses_example"));
    }
}
