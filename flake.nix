{
  description = "Tanken (探検) — GPU file manager for macOS and Linux";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-25.11";
    crate2nix.url = "github:nix-community/crate2nix";
    flake-utils.url = "github:numtide/flake-utils";
    substrate = {
      url = "github:pleme-io/substrate";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    self,
    nixpkgs,
    crate2nix,
    flake-utils,
    substrate,
  }:
    (import "${substrate}/lib/rust-tool-release-flake.nix" {
      inherit nixpkgs crate2nix flake-utils;
    }) {
      toolName = "tanken";
      src = self;
      repo = "pleme-io/tanken";

      # Migration to substrate module-trio + shikumiTypedGroups.
      # Standard kekkai-style template — typed groups for
      # appearance/navigation/search/preview, withUserDaemon for the
      # `tanken daemon` background indexer, withShikumiConfig for the
      # YAML config at ~/.config/tanken/tanken.yaml.
      module = {
        description = "Tanken (探検) — GPU file manager";
        hmNamespace = "blackmatter.components";

        # Daemon: `tanken daemon` — directory watcher + search indexer.
        withUserDaemon = true;
        userDaemonSubcommand = "daemon";

        # Shikumi YAML config at ~/.config/tanken/tanken.yaml.
        withShikumiConfig = true;

        shikumiTypedGroups = {
          appearance = {
            width       = { type = "int";   default = 900;  description = "Window width in pixels."; };
            height      = { type = "int";   default = 600;  description = "Window height in pixels."; };
            font_size   = { type = "float"; default = 14.0; description = "Font size in points."; };
            opacity     = { type = "float"; default = 0.95; description = "Background opacity (0.0-1.0)."; };
            show_hidden = { type = "bool";  default = false; description = "Show hidden files (dotfiles)."; };
            icon_size   = { type = "int";   default = 24;   description = "Icon size in pixels."; };
          };

          navigation = {
            default_path = { type = "str";       default = "~"; description = "Default directory to open."; };
            bookmarks    = {
              type = "listOfStr";
              default = [ "~/Documents" "~/Downloads" "~/Desktop" ];
              description = "Bookmarked directories shown in sidebar.";
            };
            show_sidebar = { type = "bool"; default = true; description = "Show the sidebar with bookmarks."; };
          };

          search = {
            index_dirs = {
              type = "listOfStr";
              default = [ "~" ];
              description = "Directories to index for fast search.";
            };
            exclude_patterns = {
              type = "listOfStr";
              default = [ "*.DS_Store" "node_modules" ".git" ];
              description = "Glob patterns to exclude from indexing.";
            };
          };

          preview = {
            enabled             = { type = "bool"; default = true; description = "Enable file preview panel."; };
            max_file_size_mb    = { type = "int";  default = 10;   description = "Maximum file size for preview (MB)."; };
            syntax_highlighting = { type = "bool"; default = true; description = "Enable syntax highlighting in preview."; };
          };
        };

        # Bespoke top-level options. The legacy module exposed
        # daemon.{watch_dirs,index_interval_secs} alongside daemon.enable;
        # the trio's withUserDaemon owns daemon.{enable,extraArgs,environment},
        # so we move the bespoke daemon settings into a dedicated typed
        # group `daemon_extra` which serializes into the YAML alongside
        # the trio's withUserDaemon options. This keeps `daemon.enable`
        # at the canonical trio location.
        extraHmOptions = {
          extraSettings = nixpkgs.lib.mkOption {
            type = nixpkgs.lib.types.attrs;
            default = { };
            description = "Additional raw settings merged on top of the typed YAML.";
          };
          daemon_settings = {
            watch_dirs = nixpkgs.lib.mkOption {
              type = nixpkgs.lib.types.listOf nixpkgs.lib.types.str;
              default = [ "~" ];
              description = "Directories to watch for file changes (daemon).";
            };
            index_interval_secs = nixpkgs.lib.mkOption {
              type = nixpkgs.lib.types.int;
              default = 300;
              description = "Interval between full index refreshes (daemon).";
            };
          };
        };

        # Merge daemon_settings + extraSettings into the YAML payload
        # under the `daemon` key (matches the legacy YAML shape).
        extraHmConfigFn = { cfg, lib, ... }:
          let
            daemonExtras =
              if cfg.daemon.enable
              then {
                daemon = {
                  enable = true;
                  watch_dirs = cfg.daemon_settings.watch_dirs;
                  index_interval_secs = cfg.daemon_settings.index_interval_secs;
                };
              }
              else { };
            extras = daemonExtras // cfg.extraSettings;
          in lib.mkIf (extras != { }) {
            services.tanken.settings = extras;
          };
      };
    };
}
