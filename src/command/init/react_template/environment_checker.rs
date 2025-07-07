use std::env;
use std::path::{Path, PathBuf};
use std::fs;
use anyhow::Result;

pub struct SafeEnvironmentChecker;

impl SafeEnvironmentChecker {
    /// Check if Node.js is available without spawning processes
    pub fn check_node_exists() -> bool {
        // Check common installation paths
        let common_paths = if cfg!(windows) {
            vec![
                "C:\\Program Files\\nodejs\\node.exe",
                "C:\\Program Files (x86)\\nodejs\\node.exe",
            ]
        } else {
            vec![
                "/usr/local/bin/node",
                "/usr/bin/node",
                "/opt/homebrew/bin/node",
                "/home/linuxbrew/.linuxbrew/bin/node",
            ]
        };

        for path in common_paths {
            if std::path::Path::new(path).exists() {
                return true;
            }
        }

        // Check PATH environment variable
        if let Ok(path_var) = env::var("PATH") {
            let separator = if cfg!(windows) { ';' } else { ':' };
            for path in path_var.split(separator) {
                let node_path = PathBuf::from(path).join(if cfg!(windows) { "node.exe" } else { "node" });
                if node_path.exists() {
                    return true;
                }
            }
        }

        false
    }

    /// Check if npm is available without spawning processes
    pub fn check_npm_exists() -> bool {
        // Check common installation paths
        let common_paths = if cfg!(windows) {
            vec![
                "C:\\Program Files\\nodejs\\npm.cmd",
                "C:\\Program Files (x86)\\nodejs\\npm.cmd",
            ]
        } else {
            vec![
                "/usr/local/bin/npm",
                "/usr/bin/npm",
                "/opt/homebrew/bin/npm",
                "/home/linuxbrew/.linuxbrew/bin/npm",
            ]
        };

        for path in common_paths {
            if std::path::Path::new(path).exists() {
                return true;
            }
        }

        // Check PATH environment variable
        if let Ok(path_var) = env::var("PATH") {
            let separator = if cfg!(windows) { ';' } else { ':' };
            for path in path_var.split(separator) {
                let npm_path = PathBuf::from(path).join(if cfg!(windows) { "npm.cmd" } else { "npm" });
                if npm_path.exists() {
                    return true;
                }
            }
        }

        false
    }

    /// Create a .tool-versions file for asdf users
    pub fn create_tool_versions(project_dir: &Path) -> Result<()> {
        let content = r#"nodejs 20.11.0
"#;
        fs::write(project_dir.join(".tool-versions"), content)?;
        Ok(())
    }

    /// Create .nvmrc for nvm users  
    pub fn create_nvmrc(project_dir: &Path) -> Result<()> {
        fs::write(project_dir.join(".nvmrc"), "20.11.0\n")?;
        Ok(())
    }

    /// Create VS Code configuration for the project
    pub fn create_vscode_config(project_dir: &Path) -> Result<()> {
        let vscode_dir = project_dir.join(".vscode");
        fs::create_dir_all(&vscode_dir)?;

        let extensions = serde_json::json!({
            "recommendations": [
                "ms-vscode.vscode-typescript-next",
                "apollographql.vscode-apollo",
                "bradlc.vscode-tailwindcss",
                "esbenp.prettier-vscode",
                "ms-vscode.vscode-eslint"
            ]
        });

        fs::write(
            vscode_dir.join("extensions.json"),
            serde_json::to_string_pretty(&extensions)?
        )?;

        let settings = serde_json::json!({
            "typescript.preferences.quoteStyle": "single",
            "editor.formatOnSave": true,
            "editor.defaultFormatter": "esbenp.prettier-vscode",
            "apollographql.codegenTagName": "gql"
        });

        fs::write(
            vscode_dir.join("settings.json"),
            serde_json::to_string_pretty(&settings)?
        )?;

        Ok(())
    }

    /// Detect package manager preferences
    pub fn detect_package_manager() -> PackageManager {
        // Check for lockfiles to determine preferred package manager
        if std::path::Path::new("package-lock.json").exists() {
            return PackageManager::Npm;
        }
        if std::path::Path::new("yarn.lock").exists() {
            return PackageManager::Yarn;
        }
        if std::path::Path::new("pnpm-lock.yaml").exists() {
            return PackageManager::Pnpm;
        }
        if std::path::Path::new("bun.lockb").exists() {
            return PackageManager::Bun;
        }

        // Check if alternatives are available in PATH
        if let Ok(path_var) = env::var("PATH") {
            let separator = if cfg!(windows) { ';' } else { ':' };
            for path in path_var.split(separator) {
                let path_buf = PathBuf::from(path);
                if path_buf.join("yarn").exists() || path_buf.join("yarn.cmd").exists() {
                    return PackageManager::Yarn;
                }
                if path_buf.join("pnpm").exists() || path_buf.join("pnpm.cmd").exists() {
                    return PackageManager::Pnpm;
                }
                if path_buf.join("bun").exists() || path_buf.join("bun.exe").exists() {
                    return PackageManager::Bun;
                }
            }
        }

        // Default to npm
        PackageManager::Npm
    }
}

#[derive(Debug, Clone)]
pub enum PackageManager {
    Npm,
    Yarn,
    Pnpm,
    Bun,
}

impl PackageManager {
    pub fn install_command(&self) -> &'static str {
        match self {
            PackageManager::Npm => "npm install",
            PackageManager::Yarn => "yarn install",
            PackageManager::Pnpm => "pnpm install",
            PackageManager::Bun => "bun install",
        }
    }

    pub fn dev_command(&self) -> &'static str {
        match self {
            PackageManager::Npm => "npm run dev",
            PackageManager::Yarn => "yarn dev",
            PackageManager::Pnpm => "pnpm dev",
            PackageManager::Bun => "bun dev",
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            PackageManager::Npm => "npm",
            PackageManager::Yarn => "yarn",
            PackageManager::Pnpm => "pnpm",
            PackageManager::Bun => "bun",
        }
    }
}