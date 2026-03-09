# Tanken (探検) — GPU File Manager

Crate: `tanken` | Binary: `tanken` | Config app name: `tanken`

GPU-rendered file manager with fast navigation, file previews, and vim-modal
keybindings. Uses sakuin (tantivy) for indexed search and shikumi for
hot-reloadable configuration.

## Build & Test

```bash
cargo build                    # compile
cargo test --lib               # unit tests
cargo run                      # launch GUI
cargo run -- daemon            # start file-watching daemon
cargo run -- /path/to/dir      # open specific directory
```

Nix build:
```bash
nix build                     # build via substrate rust-tool-release-flake
nix run                       # run
nix run .#regenerate           # regenerate Cargo.nix after Cargo.toml changes
```

## Competitive Position

| Competitor | Stack | Our advantage |
|-----------|-------|---------------|
| **Yazi** | Rust, TUI, Lua plugins | GPU-rendered (not terminal cells), Rhai scripting, MCP-drivable |
| **lf** | Go, vim-like, TUI | Full GPU UI, richer previews, MCP automation |
| **ranger** | Python, TUI, columns | GPU rendering, native performance, Nix-configured |
| **nnn** | C, minimal, TUI | Full-featured with GPU rendering, plugin ecosystem |
| **Thunar** | C/GTK, XFCE | Vim-modal, scriptable, MCP, not GTK-dependent |

Unique value: GPU file previews (images, syntax-highlighted code, PDF), MCP for
AI-driven file workflows, vim-modal navigation, and Rhai plugin system.

## Architecture

### Module Map

```
src/
  main.rs                      ← CLI entry point (clap: open [path], daemon)
  lib.rs                       ← Library root (re-exports config + platform)
  config.rs                    ← TankenConfig via shikumi

  platform/
    mod.rs                     ← Platform trait definitions (FileOperations, FileEntry, FileInfo)
    macos/
      mod.rs                   ← macOS file operations (NSFileManager-based)

  fs/                          ← (planned) Filesystem operations
    mod.rs                     ← Async directory listing, file metadata, watch
    watcher.rs                 ← File system event watcher (notify crate)
    trash.rs                   ← Trash/recycle bin operations (platform trait)

  navigation/                  ← (planned) Navigation state machine
    mod.rs                     ← NavigationState: current dir, cursor, selection
    miller.rs                  ← Miller columns layout (parent | current | preview)
    breadcrumb.rs              ← Path breadcrumb with clickable segments
    jump.rs                    ← Jump-to-directory (z-like frecency)

  preview/                     ← (planned) File preview engine
    mod.rs                     ← PreviewEngine: dispatch by file type
    text.rs                    ← Text preview with syntax highlighting (mojiban)
    image.rs                   ← Image preview as GPU texture (garasu)
    pdf.rs                     ← PDF first-page preview
    archive.rs                 ← Archive content listing (tar, zip)
    directory.rs               ← Directory preview (file count, size summary)

  operations/                  ← (planned) File operations
    mod.rs                     ← OperationManager: queue, progress, undo
    copy.rs                    ← Async file copy with progress
    move_op.rs                 ← Async file move
    delete.rs                  ← Delete / move to trash
    rename.rs                  ← Single rename + bulk rename
    create.rs                  ← Create file / directory

  search/                      ← (planned) Search
    mod.rs                     ← SearchEngine: sakuin index + live grep
    index.rs                   ← sakuin (tantivy) file metadata index
    content.rs                 ← Content search (ripgrep-like)
    fuzzy.rs                   ← Fuzzy filename matching

  bookmarks/                   ← (planned) Bookmarks and recent locations
    mod.rs                     ← BookmarkManager: saved dirs, recent, frecency

  render/                      ← (planned) GPU UI
    mod.rs                     ← TankenRenderer: madori RenderCallback
    file_list.rs               ← File list column (name, size, date, permissions)
    preview_pane.rs            ← Preview rendering area
    status_bar.rs              ← Bottom bar (path, selection count, free space)
    breadcrumb.rs              ← Breadcrumb path rendering

  mcp/                         ← (planned) MCP server via kaname
    mod.rs                     ← TankenMcp server struct
    tools.rs                   ← Tool implementations

  scripting/                   ← (planned) Rhai scripting via soushi
    mod.rs                     ← Engine setup, tanken.* API registration

module/
  default.nix                  ← HM module (blackmatter.components.tanken)
```

### Data Flow

```
Filesystem
    │
    ▼
FileOperations trait (platform-specific)
    │
    ▼
FileEntry[] ──▸ NavigationState (cursor, selection, sort, filter)
    │                    │
    │                    ▼
    │            PreviewEngine ──▸ GPU texture / styled text / listing
    │                    │
    └────────────────────┴──▸ GPU Render (file list + preview pane + status bar)
                                    │
                              Input Events (awase hotkeys)
                                    │
                              OperationManager (copy, move, delete queue)
```

### Platform Isolation

The `FileOperations` trait abstracts platform-specific filesystem access:

| Trait Method | Purpose |
|-------------|---------|
| `list_directory(path)` | List directory contents as `FileEntry[]` |
| `get_info(path)` | Detailed file info (size, permissions, dates, type) |
| `open(path)` | Open file with system default handler |
| `trash(path)` | Move to system trash/recycle bin |
| `create_dir(path)` | Create directory |
| `create_file(path)` | Create empty file |

Implementations: `MacOSFileOperations` (done), `LinuxFileOperations` (planned).

### Current Implementation Status

**Done:**
- `config.rs` — shikumi integration with appearance/navigation/search/preview/daemon sections
- `platform/mod.rs` — Platform trait definitions (`FileOperations`, `FileEntry`, `FileInfo`)
- `platform/macos/mod.rs` — macOS file operations
- `main.rs` — CLI entry point with GUI + daemon subcommands
- `lib.rs` — Library root
- `module/default.nix` — Full HM module with typed options + daemon service
- `flake.nix` — substrate rust-tool-release-flake + HM module

**Not started:**
- GUI rendering via madori/garasu/egaku
- File preview engine (text, image, PDF, archive)
- Navigation state machine (Miller columns, breadcrumb, jump)
- File operations (copy, move, delete, rename with progress/undo)
- Content search and fuzzy matching
- Bookmarks and recent locations
- MCP server via kaname
- Rhai scripting via soushi
- Hotkey system via awase

## Configuration

Uses **shikumi** for config discovery and hot-reload:
- Config file: `~/.config/tanken/tanken.yaml`
- Env override: `$TANKEN_CONFIG`
- Env prefix: `TANKEN_` (e.g., `TANKEN_APPEARANCE__SHOW_HIDDEN=true`)
- Hot-reload on file change (nix-darwin symlink aware)

### Config Schema

```yaml
appearance:
  width: 900
  height: 600
  font_size: 14.0
  opacity: 0.95
  show_hidden: false
  icon_size: 24

navigation:
  default_path: "~"
  bookmarks:
    - "~/Documents"
    - "~/Downloads"
    - "~/Desktop"
    - "~/code"
  show_sidebar: true
  layout: "miller"                 # miller | single | dual

search:
  index_dirs: ["~"]
  exclude_patterns: ["*.DS_Store", "node_modules", ".git", "target"]

preview:
  enabled: true
  max_file_size_mb: 10
  syntax_highlighting: true
  image_max_resolution: 2048       # max dimension for image preview texture

sort:
  field: "name"                    # name | size | modified | type
  direction: "asc"                 # asc | desc
  dirs_first: true

daemon:
  enable: false
  watch_dirs: ["~"]
  index_interval_secs: 300
```

## Shared Library Integration

| Library | Usage |
|---------|-------|
| **shikumi** | Config discovery + hot-reload (`TankenConfig`) |
| **sakuin** | Search index (tantivy wrapper for file metadata indexing) |
| **garasu** | GPU rendering for file list, preview pane, status bar |
| **madori** | App framework (event loop, render loop, input dispatch) |
| **egaku** | Widgets (list view, split pane, text input, breadcrumb, modal) |
| **mojiban** | Syntax-highlighted text preview |
| **irodzuki** | Theme: base16 to GPU uniforms |
| **hasami** | Clipboard (copy file paths, paste in rename) |
| **tsunagu** | Daemon mode for file watcher/indexer |
| **kaname** | MCP server framework |
| **soushi** | Rhai scripting engine |
| **awase** | Hotkey system for vim-modal navigation |
| **tsuuchi** | Notifications (operation complete, errors) |

## MCP Server (kaname)

Standard tools: `status`, `config_get`, `config_set`, `version`

App-specific tools:
- `list_dir(path)` — list directory contents with metadata
- `get_info(path)` — detailed file info
- `copy(src, dst)` — copy file/directory
- `move(src, dst)` — move file/directory
- `delete(path, trash?)` — delete or trash file
- `rename(old, new)` — rename file
- `create_dir(path)` — create directory
- `search(query, path?, content?)` — search files by name or content
- `preview(path)` — get file preview (text content or metadata)
- `open(path)` — open file with system handler
- `get_cwd()` — current working directory in the file manager
- `bookmark_add(path)` — add bookmark
- `recent_dirs()` — recent directories

## Rhai Scripting (soushi)

Scripts from `~/.config/tanken/scripts/*.rhai`

```rhai
// Available API:
tanken.cd("/path/to/dir")          // change directory
tanken.ls()                        // -> [{name, size, modified, is_dir}]
tanken.copy("src", "dst")          // copy file/dir
tanken.move("src", "dst")          // move file/dir
tanken.delete("path")              // delete (to trash)
tanken.rename("old", "new")        // rename
tanken.search("query")             // -> [{path, name, score}]
tanken.preview("path")             // -> file content or metadata
tanken.open("path")                // open with system handler
tanken.bookmark("path")            // add to bookmarks
tanken.selected()                  // -> [selected file paths]
tanken.mkdir("name")               // create directory
tanken.touch("name")               // create file
```

Event hooks: `on_startup`, `on_shutdown`, `on_cd(path)`, `on_select(path)`,
`on_open(path)`, `on_copy(src, dst)`, `on_delete(path)`

Example: auto-preview markdown files:
```rhai
fn on_select(path) {
    if path.ends_with(".md") {
        tanken.preview(path);
    }
}
```

## Hotkey System (awase)

### Modes

**Normal** (default — file list navigation):
| Key | Action |
|-----|--------|
| `h` | Go to parent directory |
| `j/k` | Navigate files up/down |
| `l` | Enter directory / open file |
| `Enter` | Open file with system handler |
| `Space` | Toggle selection |
| `gg` | Jump to first file |
| `G` | Jump to last file |
| `.` | Toggle hidden files |
| `p` | Paste (copy or move depending on yank/cut) |
| `y` | Yank (copy) selected files |
| `d` | Cut selected files |
| `dd` | Delete selected files (to trash) |
| `r` | Rename file under cursor |
| `o` | Create new file |
| `O` | Create new directory |
| `/` | Incremental filename search |
| `?` | Content search (grep) |
| `n/N` | Next/previous search match |
| `Tab` | Switch pane (Miller columns) |
| `q` | Quit |
| `:` | Command mode |

**Visual** (multi-select mode — `v` to enter):
| Key | Action |
|-----|--------|
| `j/k` | Extend selection up/down |
| `y` | Copy all selected |
| `d` | Cut all selected |
| `Esc` | Cancel selection |

**Command** (`:` prefix):
- `:cd <path>` — change directory
- `:mkdir <name>` — create directory
- `:touch <name>` — create file
- `:rename` — enter rename mode for selected
- `:chmod <mode>` — change permissions
- `:search <query>` — search files
- `:sort name|size|modified|type` — change sort
- `:bookmark` — bookmark current directory
- `:open <path>` — open specific path

## Nix Integration

### Flake Exports
- Multi-platform packages via substrate `rust-tool-release-flake.nix`
- `overlays.default` — `pkgs.tanken`
- `homeManagerModules.default` — `blackmatter.components.tanken`
- `devShells` — dev environment with rustc, cargo

### HM Module

Namespace: `blackmatter.components.tanken`

Fully implemented with typed options:
- `enable` — install package + generate config
- `package` — override package
- `appearance.{width, height, font_size, opacity, show_hidden, icon_size}`
- `navigation.{default_path, bookmarks, show_sidebar}`
- `search.{index_dirs, exclude_patterns}`
- `preview.{enabled, max_file_size_mb, syntax_highlighting}`
- `daemon.{enable, watch_dirs, index_interval_secs}` — launchd/systemd service
- `extraSettings` — raw attrset escape hatch

YAML generated via `lib.generators.toYAML` -> `xdg.configFile."tanken/tanken.yaml"`.
Uses substrate's `hm-service-helpers.nix` for `mkLaunchdService`/`mkSystemdService`.

## Navigation Design

### Miller Columns (default layout)

```
┌─────────────┬──────────────────┬─────────────────────┐
│ Parent Dir  │ Current Dir      │ Preview Pane        │
│             │                  │                     │
│ > Documents │ > src/           │ fn main() {         │
│   Downloads │   tests/         │   let app = App..   │
│   Desktop   │ * Cargo.toml     │   app.run();        │
│   code/     │   README.md      │ }                   │
│             │   .gitignore     │                     │
│             │                  │                     │
├─────────────┴──────────────────┴─────────────────────┤
│ ~/code/github/pleme-io/tanken  3 selected  12 items  │
└──────────────────────────────────────────────────────┘
```

- Left column: parent directory (read-only, for context)
- Center column: current directory (navigable, selectable)
- Right column: preview of item under cursor
- Status bar: current path, selection count, item count, free space

### Preview Engine Priority

For the file under cursor, preview dispatches by type:
1. **Directory** — item count, total size, child listing
2. **Text/Code** — syntax-highlighted content via mojiban (first N lines)
3. **Image** — GPU texture via garasu (JPEG, PNG, WebP, GIF first frame)
4. **PDF** — first page rendered as image
5. **Archive** — file listing (tar/zip content table)
6. **Binary** — hex dump header + file metadata
7. **Unsupported** — file metadata only (size, permissions, dates)

## Design Constraints

- **Platform trait** — all filesystem operations go through `FileOperations` trait
- **Async operations** — copy, move, delete are async with progress tracking and cancellation
- **Trash by default** — `dd` moves to system trash, not permanent delete; permanent delete requires `:delete!`
- **No file content mutation** — tanken does not edit files, only manages (copy/move/delete/rename)
- **Preview size limits** — text preview capped at N lines, image preview capped at configurable resolution
- **Index is optional** — search works without daemon (falls back to live walk), index makes it instant
- **GPU rendering** — all UI via garasu/madori/egaku, Miller columns are egaku SplitPane widgets
