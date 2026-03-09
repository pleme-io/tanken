//! Keyboard input handling with vim-style modal navigation.
//!
//! Three modes: Normal (default), Visual (multi-select), Command (`:` prefix).
//! Key sequences like `gg` and `dd` are supported via a pending-key buffer.
//! Uses `awase::Hotkey` for key binding definitions.

/// Input mode for the file manager.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Default mode: navigate files, single operations.
    Normal,
    /// Multi-select mode: extend selection with movement.
    Visual,
    /// Command input mode (`:` prefix).
    Command,
    /// Incremental search mode (`/` prefix).
    Search,
    /// Rename mode: editing the current file name.
    Rename,
    /// Create mode: typing a new file/dir name.
    Create { is_dir: bool },
}

/// Actions that input events can produce.
#[derive(Debug, Clone)]
pub enum Action {
    // Navigation
    CursorUp,
    CursorDown,
    CursorTop,
    CursorBottom,
    EnterDir,
    ParentDir,
    OpenFile,

    // Selection
    ToggleSelect,
    SelectAll,
    ClearSelection,

    // File operations
    Yank,
    Cut,
    Paste,
    Delete,
    RenameStart,
    RenameConfirm(String),
    RenameCancel,
    CreateFile(String),
    CreateDir(String),
    CreateStart { is_dir: bool },

    // View
    ToggleHidden,
    TogglePane,
    Refresh,

    // Search
    SearchStart,
    SearchUpdate(String),
    SearchConfirm,
    SearchCancel,
    SearchNext,
    SearchPrev,

    // Tabs
    NewTab,
    CloseTab,
    NextTab,
    PrevTab,

    // Bookmarks
    BookmarkAdd,
    BookmarkGo(usize),

    // Sort
    SortByName,
    SortBySize,
    SortByModified,
    SortByExtension,

    // Command
    CommandStart,
    CommandExecute(String),
    CommandCancel,

    // Mode switching
    EnterVisual,
    ExitVisual,

    // App
    Quit,
    None,
}

/// Convert a crossterm `KeyCode` to an `awase::Key`.
fn to_awase_key(code: &crossterm::event::KeyCode) -> Option<awase::Key> {
    use crossterm::event::KeyCode;
    match code {
        KeyCode::Char(c) => match c.to_ascii_lowercase() {
            'a' => Some(awase::Key::A),
            'b' => Some(awase::Key::B),
            'c' => Some(awase::Key::C),
            'd' => Some(awase::Key::D),
            'e' => Some(awase::Key::E),
            'f' => Some(awase::Key::F),
            'g' => Some(awase::Key::G),
            'h' => Some(awase::Key::H),
            'i' => Some(awase::Key::I),
            'j' => Some(awase::Key::J),
            'k' => Some(awase::Key::K),
            'l' => Some(awase::Key::L),
            'm' => Some(awase::Key::M),
            'n' => Some(awase::Key::N),
            'o' => Some(awase::Key::O),
            'p' => Some(awase::Key::P),
            'q' => Some(awase::Key::Q),
            'r' => Some(awase::Key::R),
            's' => Some(awase::Key::S),
            't' => Some(awase::Key::T),
            'u' => Some(awase::Key::U),
            'v' => Some(awase::Key::V),
            'w' => Some(awase::Key::W),
            'x' => Some(awase::Key::X),
            'y' => Some(awase::Key::Y),
            'z' => Some(awase::Key::Z),
            ' ' => Some(awase::Key::Space),
            _ => Option::None,
        },
        KeyCode::Enter => Some(awase::Key::Return),
        KeyCode::Esc => Some(awase::Key::Escape),
        KeyCode::Tab => Some(awase::Key::Tab),
        KeyCode::Backspace => Some(awase::Key::Backspace),
        KeyCode::Delete => Some(awase::Key::Delete),
        KeyCode::Up => Some(awase::Key::Up),
        KeyCode::Down => Some(awase::Key::Down),
        KeyCode::Left => Some(awase::Key::Left),
        KeyCode::Right => Some(awase::Key::Right),
        _ => Option::None,
    }
}

/// Convert crossterm modifiers to awase modifiers.
fn to_awase_modifiers(mods: &crossterm::event::KeyModifiers) -> awase::Modifiers {
    use crossterm::event::KeyModifiers;
    let mut result = awase::Modifiers::NONE;
    if mods.contains(KeyModifiers::CONTROL) {
        result = result | awase::Modifiers::CTRL;
    }
    if mods.contains(KeyModifiers::ALT) {
        result = result | awase::Modifiers::ALT;
    }
    if mods.contains(KeyModifiers::SHIFT) {
        result = result | awase::Modifiers::SHIFT;
    }
    if mods.contains(KeyModifiers::SUPER) {
        result = result | awase::Modifiers::CMD;
    }
    result
}

/// Input handler with mode and pending key state.
#[derive(Debug)]
pub struct InputHandler {
    pub mode: Mode,
    /// Pending key for multi-key sequences (e.g., `g` in `gg`).
    pending_key: Option<char>,
    /// Text buffer for command/search/rename/create modes.
    pub input_buffer: String,
}

impl InputHandler {
    #[must_use]
    pub fn new() -> Self {
        Self {
            mode: Mode::Normal,
            pending_key: None,
            input_buffer: String::new(),
        }
    }

    /// Process a key event and return the resulting action.
    pub fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> Action {
        match self.mode {
            Mode::Normal => self.handle_normal(key),
            Mode::Visual => self.handle_visual(key),
            Mode::Command => self.handle_text_input(key, Mode::Command),
            Mode::Search => self.handle_text_input(key, Mode::Search),
            Mode::Rename => self.handle_text_input(key, Mode::Rename),
            Mode::Create { .. } => self.handle_text_input(key, self.mode),
        }
    }

    fn handle_normal(&mut self, key: crossterm::event::KeyEvent) -> Action {
        use crossterm::event::{KeyCode, KeyModifiers};

        // Check for pending key sequences
        if let Some(pending) = self.pending_key.take() {
            return match (pending, key.code) {
                ('g', KeyCode::Char('g')) => Action::CursorTop,
                ('d', KeyCode::Char('d')) => Action::Delete,
                _ => Action::None,
            };
        }

        // Build awase hotkey for modifier-based bindings
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            // Handle keys directly for punctuation not in awase::Key
            match key.code {
                KeyCode::Char(']') => return Action::NextTab,
                KeyCode::Char('[') => return Action::PrevTab,
                _ => {}
            }
            if let Some(awase_key) = to_awase_key(&key.code) {
                let hotkey = awase::Hotkey::new(awase::Modifiers::CTRL, awase_key);
                return match hotkey.key {
                    awase::Key::T => Action::NewTab,
                    awase::Key::W => Action::CloseTab,
                    awase::Key::C => Action::Quit,
                    _ => Action::None,
                };
            }
        }

        match key.code {
            // Navigation
            KeyCode::Char('j') | KeyCode::Down => Action::CursorDown,
            KeyCode::Char('k') | KeyCode::Up => Action::CursorUp,
            KeyCode::Char('l') | KeyCode::Right => Action::EnterDir,
            KeyCode::Char('h') | KeyCode::Left => Action::ParentDir,
            KeyCode::Enter => Action::OpenFile,
            KeyCode::Char('G') => Action::CursorBottom,
            KeyCode::Char('g') => {
                self.pending_key = Some('g');
                Action::None
            }

            // Selection
            KeyCode::Char(' ') => Action::ToggleSelect,
            KeyCode::Char('v') => {
                self.mode = Mode::Visual;
                Action::EnterVisual
            }

            // File operations
            KeyCode::Char('y') => Action::Yank,
            KeyCode::Char('d') => {
                self.pending_key = Some('d');
                Action::None
            }
            KeyCode::Char('p') => Action::Paste,
            KeyCode::Char('r') => {
                self.mode = Mode::Rename;
                self.input_buffer.clear();
                Action::RenameStart
            }
            KeyCode::Char('o') => {
                self.mode = Mode::Create { is_dir: false };
                self.input_buffer.clear();
                Action::CreateStart { is_dir: false }
            }
            KeyCode::Char('O') => {
                self.mode = Mode::Create { is_dir: true };
                self.input_buffer.clear();
                Action::CreateStart { is_dir: true }
            }

            // View
            KeyCode::Char('.') => Action::ToggleHidden,
            KeyCode::Tab => Action::TogglePane,
            KeyCode::Char('R') => Action::Refresh,

            // Search
            KeyCode::Char('/') => {
                self.mode = Mode::Search;
                self.input_buffer.clear();
                Action::SearchStart
            }
            KeyCode::Char('n') => Action::SearchNext,
            KeyCode::Char('N') => Action::SearchPrev,

            // Bookmarks
            KeyCode::Char('b') => Action::BookmarkAdd,

            // Sort
            KeyCode::Char('s') => Action::SortByName,
            KeyCode::Char('S') => Action::SortBySize,

            // Command
            KeyCode::Char(':') => {
                self.mode = Mode::Command;
                self.input_buffer.clear();
                Action::CommandStart
            }

            // Quit
            KeyCode::Char('q') => Action::Quit,

            _ => Action::None,
        }
    }

    fn handle_visual(&mut self, key: crossterm::event::KeyEvent) -> Action {
        use crossterm::event::KeyCode;

        match key.code {
            KeyCode::Char('j') | KeyCode::Down => Action::CursorDown,
            KeyCode::Char('k') | KeyCode::Up => Action::CursorUp,
            KeyCode::Char('y') => {
                self.mode = Mode::Normal;
                Action::Yank
            }
            KeyCode::Char('d') => {
                self.mode = Mode::Normal;
                Action::Cut
            }
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                Action::ExitVisual
            }
            _ => Action::None,
        }
    }

    fn handle_text_input(&mut self, key: crossterm::event::KeyEvent, mode: Mode) -> Action {
        use crossterm::event::KeyCode;

        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.input_buffer.clear();
                match mode {
                    Mode::Search => Action::SearchCancel,
                    Mode::Command => Action::CommandCancel,
                    Mode::Rename => Action::RenameCancel,
                    Mode::Create { .. } => Action::None,
                    _ => Action::None,
                }
            }
            KeyCode::Enter => {
                let text = self.input_buffer.clone();
                self.mode = Mode::Normal;
                self.input_buffer.clear();
                match mode {
                    Mode::Search => Action::SearchConfirm,
                    Mode::Command => Action::CommandExecute(text),
                    Mode::Rename => Action::RenameConfirm(text),
                    Mode::Create { is_dir: true } => Action::CreateDir(text),
                    Mode::Create { is_dir: false } => Action::CreateFile(text),
                    _ => Action::None,
                }
            }
            KeyCode::Backspace => {
                self.input_buffer.pop();
                if matches!(mode, Mode::Search) {
                    Action::SearchUpdate(self.input_buffer.clone())
                } else {
                    Action::None
                }
            }
            KeyCode::Char(c) => {
                self.input_buffer.push(c);
                if matches!(mode, Mode::Search) {
                    Action::SearchUpdate(self.input_buffer.clone())
                } else {
                    Action::None
                }
            }
            _ => Action::None,
        }
    }
}

impl Default for InputHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse a command string (from `:` mode) into an action.
#[must_use]
pub fn parse_command(cmd: &str) -> Action {
    let parts: Vec<&str> = cmd.trim().splitn(2, ' ').collect();
    let command = parts.first().copied().unwrap_or("");
    let arg = parts.get(1).copied().unwrap_or("");

    match command {
        "q" | "quit" => Action::Quit,
        "cd" => {
            if arg.is_empty() {
                Action::None
            } else {
                let path = crate::bookmarks::expand_tilde(arg);
                Action::CommandExecute(format!("cd {}", path.display()))
            }
        }
        "mkdir" => {
            if arg.is_empty() {
                Action::None
            } else {
                Action::CreateDir(arg.to_string())
            }
        }
        "touch" => {
            if arg.is_empty() {
                Action::None
            } else {
                Action::CreateFile(arg.to_string())
            }
        }
        "sort" => match arg {
            "name" => Action::SortByName,
            "size" => Action::SortBySize,
            "modified" | "date" => Action::SortByModified,
            "ext" | "type" => Action::SortByExtension,
            _ => Action::None,
        },
        "bookmark" | "bm" => Action::BookmarkAdd,
        "refresh" => Action::Refresh,
        _ => Action::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_command_quit() {
        matches!(parse_command("q"), Action::Quit);
        matches!(parse_command("quit"), Action::Quit);
    }

    #[test]
    fn parse_command_mkdir() {
        match parse_command("mkdir test_dir") {
            Action::CreateDir(name) => assert_eq!(name, "test_dir"),
            _ => panic!("expected CreateDir"),
        }
    }

    #[test]
    fn parse_command_sort() {
        matches!(parse_command("sort name"), Action::SortByName);
        matches!(parse_command("sort size"), Action::SortBySize);
    }

    #[test]
    fn mode_default_is_normal() {
        let handler = InputHandler::new();
        assert_eq!(handler.mode, Mode::Normal);
    }

    #[test]
    fn awase_key_conversion() {
        use crossterm::event::KeyCode;
        assert_eq!(to_awase_key(&KeyCode::Char('a')), Some(awase::Key::A));
        assert_eq!(to_awase_key(&KeyCode::Esc), Some(awase::Key::Escape));
        assert_eq!(to_awase_key(&KeyCode::Enter), Some(awase::Key::Return));
        assert_eq!(to_awase_key(&KeyCode::Tab), Some(awase::Key::Tab));
    }

    #[test]
    fn awase_modifier_conversion() {
        use crossterm::event::KeyModifiers;
        let mods = KeyModifiers::CONTROL | KeyModifiers::SHIFT;
        let awase_mods = to_awase_modifiers(&mods);
        assert!(awase_mods.contains(awase::Modifiers::CTRL));
        assert!(awase_mods.contains(awase::Modifiers::SHIFT));
        assert!(!awase_mods.contains(awase::Modifiers::CMD));
    }
}
