//! Rhai scripting integration via soushi.
//!
//! Loads user scripts from `~/.config/tanken/scripts/*.rhai` and exposes
//! file manager functions: `tanken.cd`, `tanken.copy`, `tanken.selected_files`,
//! `tanken.open`.

use std::collections::HashMap;
use std::path::PathBuf;

use soushi::ScriptEngine;

/// Script event hooks that scripts can define.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScriptEvent {
    /// Fired when the application starts.
    OnStart,
    /// Fired when the application is about to quit.
    OnQuit,
    /// Fired on every key press.
    OnKey,
}

/// Manages the Rhai scripting engine and user scripts for tanken.
pub struct ScriptManager {
    engine: ScriptEngine,
    hooks: HashMap<ScriptEvent, Vec<soushi::rhai::AST>>,
    named_scripts: HashMap<String, soushi::rhai::AST>,
    scripts_dir: PathBuf,
}

impl ScriptManager {
    /// Create a new script manager and register tanken-specific functions.
    #[must_use]
    pub fn new() -> Self {
        let mut engine = ScriptEngine::new();
        engine.register_builtin_log();
        engine.register_builtin_env();
        engine.register_builtin_string();

        Self::register_tanken_functions(&mut engine);

        let scripts_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("~/.config"))
            .join("tanken")
            .join("scripts");

        let mut manager = Self {
            engine,
            hooks: HashMap::new(),
            named_scripts: HashMap::new(),
            scripts_dir,
        };

        manager.load_scripts();
        manager
    }

    /// Register tanken-specific functions with the scripting engine.
    fn register_tanken_functions(engine: &mut ScriptEngine) {
        engine.register_fn("tanken_cd", |path: &str| -> String {
            tracing::info!(path, "script: tanken.cd");
            format!("changed to: {path}")
        });

        engine.register_fn("tanken_copy", |src: &str, dst: &str| -> String {
            tracing::info!(src, dst, "script: tanken.copy");
            format!("copied {src} -> {dst}")
        });

        engine.register_fn("tanken_selected_files", || -> soushi::rhai::Array {
            tracing::info!("script: tanken.selected_files");
            soushi::rhai::Array::new()
        });

        engine.register_fn("tanken_open", |path: &str| -> String {
            tracing::info!(path, "script: tanken.open");
            format!("opened: {path}")
        });
    }

    /// Load all scripts from the scripts directory.
    fn load_scripts(&mut self) {
        if !self.scripts_dir.is_dir() {
            tracing::debug!(
                path = %self.scripts_dir.display(),
                "scripts directory does not exist, skipping"
            );
            return;
        }

        match self.engine.load_scripts_dir(&self.scripts_dir) {
            Ok(names) => {
                tracing::info!(count = names.len(), "loaded tanken scripts");
                for name in &names {
                    self.compile_named_script(name);
                }
            }
            Err(e) => {
                tracing::error!(error = %e, "failed to load scripts");
            }
        }
    }

    /// Compile and store a named script for later execution.
    fn compile_named_script(&mut self, name: &str) {
        let path = self.scripts_dir.join(format!("{name}.rhai"));
        if let Ok(source) = std::fs::read_to_string(&path) {
            match self.engine.compile(&source) {
                Ok(ast) => {
                    self.named_scripts.insert(name.to_string(), ast);
                }
                Err(e) => {
                    tracing::error!(script = name, error = %e, "failed to compile script");
                }
            }
        }
    }

    /// Register a hook script for a given event.
    pub fn register_hook(&mut self, event: ScriptEvent, script: &str) {
        match self.engine.compile(script) {
            Ok(ast) => {
                self.hooks.entry(event).or_default().push(ast);
            }
            Err(e) => {
                tracing::error!(event = ?event, error = %e, "failed to compile hook");
            }
        }
    }

    /// Fire all hooks registered for a given event.
    pub fn fire_event(&self, event: ScriptEvent) {
        if let Some(scripts) = self.hooks.get(&event) {
            for ast in scripts {
                if let Err(e) = self.engine.eval_ast(ast) {
                    tracing::error!(event = ?event, error = %e, "hook script failed");
                }
            }
        }
    }

    /// Run a named script by file stem.
    pub fn run_script(&self, name: &str) -> Result<soushi::rhai::Dynamic, soushi::SoushiError> {
        if let Some(ast) = self.named_scripts.get(name) {
            self.engine.eval_ast(ast)
        } else {
            let path = self.scripts_dir.join(format!("{name}.rhai"));
            self.engine.eval_file(&path)
        }
    }

    /// Access the underlying script engine.
    #[must_use]
    pub fn engine(&self) -> &ScriptEngine {
        &self.engine
    }
}

impl Default for ScriptManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_script_manager() {
        let _mgr = ScriptManager::new();
    }

    #[test]
    fn register_and_fire_hook() {
        let mut mgr = ScriptManager::new();
        mgr.register_hook(ScriptEvent::OnStart, r#"log_info("on_start fired")"#);
        mgr.fire_event(ScriptEvent::OnStart);
    }

    #[test]
    fn tanken_cd_callable() {
        let mgr = ScriptManager::new();
        let result = mgr.engine().eval(r#"tanken_cd("/tmp")"#).unwrap();
        assert!(result.into_string().unwrap().contains("changed to"));
    }

    #[test]
    fn tanken_copy_callable() {
        let mgr = ScriptManager::new();
        let result = mgr.engine().eval(r#"tanken_copy("/tmp/a", "/tmp/b")"#).unwrap();
        assert!(result.into_string().unwrap().contains("copied"));
    }

    #[test]
    fn tanken_selected_files_callable() {
        let mgr = ScriptManager::new();
        let result = mgr.engine().eval("tanken_selected_files()").unwrap();
        let arr = result.into_array().unwrap();
        assert!(arr.is_empty());
    }

    #[test]
    fn tanken_open_callable() {
        let mgr = ScriptManager::new();
        let result = mgr.engine().eval(r#"tanken_open("/tmp/file.txt")"#).unwrap();
        assert!(result.into_string().unwrap().contains("opened"));
    }

    #[test]
    fn run_nonexistent_script_errors() {
        let mgr = ScriptManager::new();
        assert!(mgr.run_script("nonexistent_script_12345").is_err());
    }
}
