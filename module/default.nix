# Tanken home-manager module — GPU file manager with typed config + daemon
#
# Namespace: blackmatter.components.tanken.*
#
# Generates YAML config from typed Nix options, loaded by shikumi at runtime.
# Supports hot-reload via symlink-aware file watching.
#
# Module factory: receives { hmHelpers } from flake.nix, returns HM module.
{ hmHelpers }:
{
  lib,
  config,
  pkgs,
  ...
}:
with lib;
let
  inherit (hmHelpers) mkLaunchdService mkSystemdService;
  cfg = config.blackmatter.components.tanken;
  isDarwin = pkgs.stdenv.isDarwin;

  logDir =
    if isDarwin then "${config.home.homeDirectory}/Library/Logs"
    else "${config.home.homeDirectory}/.local/share/tanken/logs";

  # -- YAML config generation --------------------------------------------------
  settingsAttr = let
    appearance = filterAttrs (_: v: v != null) {
      inherit (cfg.appearance) width height font_size opacity show_hidden icon_size;
    };

    navigation = filterAttrs (_: v: v != null) {
      default_path = cfg.navigation.default_path;
      bookmarks = if cfg.navigation.bookmarks == [] then null else cfg.navigation.bookmarks;
      show_sidebar = cfg.navigation.show_sidebar;
    };

    search = filterAttrs (_: v: v != null) {
      index_dirs = if cfg.search.index_dirs == [] then null else cfg.search.index_dirs;
      exclude_patterns = if cfg.search.exclude_patterns == [] then null else cfg.search.exclude_patterns;
    };

    preview = filterAttrs (_: v: v != null) {
      inherit (cfg.preview) enabled max_file_size_mb syntax_highlighting;
    };

    daemon = optionalAttrs cfg.daemon.enable (filterAttrs (_: v: v != null) {
      enable = cfg.daemon.enable;
      watch_dirs = if cfg.daemon.watch_dirs == [] then null else cfg.daemon.watch_dirs;
      index_interval_secs = cfg.daemon.index_interval_secs;
    });
  in
    filterAttrs (_: v: v != {} && v != null) {
      inherit appearance navigation search preview daemon;
    }
    // cfg.extraSettings;

  yamlConfig = pkgs.writeText "tanken.yaml"
    (lib.generators.toYAML { } settingsAttr);
in
{
  options.blackmatter.components.tanken = {
    enable = mkEnableOption "Tanken — GPU file manager";

    package = mkOption {
      type = types.package;
      default = pkgs.tanken;
      description = "The tanken package to use.";
    };

    # -- Appearance ------------------------------------------------------------
    appearance = {
      width = mkOption {
        type = types.int;
        default = 900;
        description = "Window width in pixels.";
      };

      height = mkOption {
        type = types.int;
        default = 600;
        description = "Window height in pixels.";
      };

      font_size = mkOption {
        type = types.float;
        default = 14.0;
        description = "Font size in points.";
      };

      opacity = mkOption {
        type = types.float;
        default = 0.95;
        description = "Background opacity (0.0-1.0).";
      };

      show_hidden = mkOption {
        type = types.bool;
        default = false;
        description = "Show hidden files (dotfiles).";
      };

      icon_size = mkOption {
        type = types.int;
        default = 24;
        description = "Icon size in pixels.";
      };
    };

    # -- Navigation ------------------------------------------------------------
    navigation = {
      default_path = mkOption {
        type = types.str;
        default = "~";
        description = "Default directory to open.";
      };

      bookmarks = mkOption {
        type = types.listOf types.str;
        default = [ "~/Documents" "~/Downloads" "~/Desktop" ];
        description = "Bookmarked directories shown in sidebar.";
      };

      show_sidebar = mkOption {
        type = types.bool;
        default = true;
        description = "Show the sidebar with bookmarks.";
      };
    };

    # -- Search ----------------------------------------------------------------
    search = {
      index_dirs = mkOption {
        type = types.listOf types.str;
        default = [ "~" ];
        description = "Directories to index for fast search.";
      };

      exclude_patterns = mkOption {
        type = types.listOf types.str;
        default = [ "*.DS_Store" "node_modules" ".git" ];
        description = "Glob patterns to exclude from indexing.";
        example = [ "*.DS_Store" "node_modules" ".git" "target" ];
      };
    };

    # -- Preview ---------------------------------------------------------------
    preview = {
      enabled = mkOption {
        type = types.bool;
        default = true;
        description = "Enable file preview panel.";
      };

      max_file_size_mb = mkOption {
        type = types.int;
        default = 10;
        description = "Maximum file size for preview (MB).";
      };

      syntax_highlighting = mkOption {
        type = types.bool;
        default = true;
        description = "Enable syntax highlighting in preview.";
      };
    };

    # -- Daemon ----------------------------------------------------------------
    daemon = {
      enable = mkOption {
        type = types.bool;
        default = false;
        description = ''
          Run tanken as a persistent daemon (launchd on macOS, systemd on Linux).
          The daemon watches directories and maintains the search index.
        '';
      };

      watch_dirs = mkOption {
        type = types.listOf types.str;
        default = [ "~" ];
        description = "Directories to watch for file changes.";
      };

      index_interval_secs = mkOption {
        type = types.int;
        default = 300;
        description = "Interval in seconds between full index refreshes.";
      };
    };

    # -- Escape hatch ----------------------------------------------------------
    extraSettings = mkOption {
      type = types.attrs;
      default = {};
      description = ''
        Additional raw settings merged on top of typed options.
        Use this for experimental or newly-added config keys not yet
        covered by typed options. Values are serialized directly to YAML.
      '';
      example = {
        experimental = {
          gpu_backend = "metal";
        };
      };
    };
  };

  config = mkIf cfg.enable (mkMerge [
    # Install the package
    {
      home.packages = [ cfg.package ];
    }

    # Create log directory
    {
      home.activation.tanken-log-dir = lib.hm.dag.entryAfter [ "writeBoundary" ] ''
        run mkdir -p "${logDir}"
      '';
    }

    # YAML configuration -- always generated from typed options
    {
      xdg.configFile."tanken/tanken.yaml".source = yamlConfig;
    }

    # Darwin: launchd agent (daemon mode)
    (mkIf (cfg.daemon.enable && isDarwin)
      (mkLaunchdService {
        name = "tanken";
        label = "io.pleme.tanken";
        command = "${cfg.package}/bin/tanken";
        args = [ "daemon" ];
        logDir = logDir;
        processType = "Background";
        keepAlive = true;
      })
    )

    # Linux: systemd user service (daemon mode)
    (mkIf (cfg.daemon.enable && !isDarwin)
      (mkSystemdService {
        name = "tanken";
        description = "Tanken — file manager daemon";
        command = "${cfg.package}/bin/tanken";
        args = [ "daemon" ];
      })
    )
  ]);
}
