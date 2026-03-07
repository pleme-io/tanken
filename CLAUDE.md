# Tanken (探検) — GPU File Manager

## Build & Test

```bash
cargo build                    # compile
cargo test --lib               # unit tests
cargo run                      # launch GUI
cargo run -- daemon            # start file-watching daemon
```

## Architecture

### Pipeline

```
Filesystem → Directory Scanner → sakuin Index
                                      |
  Input Event → Navigation → Preview → GPU Render
```

### Platform Isolation (`src/platform/`)

| Trait | macOS Impl | Purpose |
|-------|------------|---------|
| `FileOperations` | `MacOSFileOperations` | List dirs, open files, trash, file info |

Linux implementations will be added under `src/platform/linux/`.

### Configuration

Uses **shikumi** for config discovery and hot-reload:
- Config file: `~/.config/tanken/tanken.yaml`
- Env override: `$TANKEN_CONFIG`
- Env vars: `TANKEN_` prefix (e.g. `TANKEN_APPEARANCE__WIDTH=1200`)
- Hot-reload on file change (nix-darwin symlink aware)

## File Map

| Path | Purpose |
|------|---------|
| `src/config.rs` | Config struct (uses shikumi) |
| `src/platform/mod.rs` | Platform trait definitions (FileOperations, FileEntry, FileInfo) |
| `src/platform/macos/mod.rs` | macOS file operations |
| `src/main.rs` | CLI entry point (GUI + daemon subcommands) |
| `src/lib.rs` | Library root |
| `module/default.nix` | HM module with typed options + daemon |

## Design Decisions

### Configuration Language: YAML
- YAML is the primary and only configuration format
- Config file: `~/.config/tanken/tanken.yaml`
- Nix HM module generates YAML via `lib.generators.toYAML` from typed options
- Typed options mirror `TankenConfig` struct: appearance, navigation, search, preview, daemon
- `extraSettings` escape hatch for raw attrset merge on top of typed options

### Nix Integration
- Flake exports: `packages`, `overlays.default`, `homeManagerModules.default`, `devShells`
- HM module at `blackmatter.components.tanken` with fully typed options:
  - `appearance.{width, height, font_size, opacity, show_hidden, icon_size}`
  - `navigation.{default_path, bookmarks, show_sidebar}`
  - `search.{index_dirs, exclude_patterns}`
  - `preview.{enabled, max_file_size_mb, syntax_highlighting}`
  - `daemon.{enable, watch_dirs, index_interval_secs}` with launchd/systemd service
  - `extraSettings` — raw attrset escape hatch
- YAML generated via `lib.generators.toYAML` -> `xdg.configFile."tanken/tanken.yaml"`
- Cross-platform: `mkLaunchdService` (macOS) + `mkSystemdService` (Linux) for daemon
- Uses substrate's `hm-service-helpers.nix` for service generation

### Cross-Platform Strategy
- Platform-specific: behind trait boundaries in `src/platform/`
- Search index: sakuin (tantivy wrapper) for file metadata
- Config: shikumi for discovery and hot-reload
