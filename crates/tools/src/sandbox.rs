use std::path::{Path, PathBuf};

use rust_claude_core::sandbox::{NetworkPolicy, SandboxAdapterAvailability, SandboxConfig};
use tokio::process::Command;

#[derive(Debug, Clone)]
pub struct SandboxCommandSpec {
    pub program: String,
    pub args: Vec<String>,
    pub current_dir: Option<PathBuf>,
    pub allowed_paths: Vec<PathBuf>,
    pub network: NetworkPolicy,
}

impl SandboxCommandSpec {
    pub fn new(
        program: impl Into<String>,
        args: Vec<String>,
        current_dir: Option<PathBuf>,
        config: &SandboxConfig,
        fallback_allowed_path: &Path,
    ) -> Self {
        let allowed_paths = if config.allowed_paths.is_empty() {
            rust_claude_core::sandbox::default_allowed_paths(fallback_allowed_path)
        } else {
            rust_claude_core::sandbox::canonicalize_allowed_paths(
                &config.allowed_paths,
                fallback_allowed_path,
                rust_claude_core::sandbox::home_dir().as_deref(),
            )
        };

        Self {
            program: program.into(),
            args,
            current_dir,
            allowed_paths,
            network: config.network,
        }
    }
}

pub trait SandboxAdapter: Send + Sync {
    fn availability(&self) -> SandboxAdapterAvailability;
    fn wrap(&self, spec: &SandboxCommandSpec) -> Result<Command, String>;
}

pub fn platform_adapter() -> Box<dyn SandboxAdapter> {
    #[cfg(target_os = "macos")]
    {
        Box::new(macos::MacSandboxAdapter)
    }
    #[cfg(target_os = "linux")]
    {
        Box::new(linux::BwrapSandboxAdapter)
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        Box::new(unsupported::UnsupportedSandboxAdapter)
    }
}

pub fn executable_available(name: &str) -> bool {
    let Some(paths) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&paths).any(|dir| dir.join(name).is_file())
}

fn apply_common(command: &mut Command, spec: &SandboxCommandSpec) {
    command.kill_on_drop(true);
    if let Some(current_dir) = &spec.current_dir {
        command.current_dir(current_dir);
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::*;

    pub struct MacSandboxAdapter;

    impl SandboxAdapter for MacSandboxAdapter {
        fn availability(&self) -> SandboxAdapterAvailability {
            if executable_available("sandbox-exec") {
                SandboxAdapterAvailability::Available {
                    name: "sandbox-exec",
                }
            } else {
                SandboxAdapterAvailability::Unavailable {
                    reason: "sandbox-exec is not available on PATH".to_string(),
                }
            }
        }

        fn wrap(&self, spec: &SandboxCommandSpec) -> Result<Command, String> {
            if !self.availability().is_available() {
                return Err("sandbox-exec is not available".to_string());
            }

            let mut command = Command::new("sandbox-exec");
            command.arg("-p").arg(profile(spec));
            command.arg(&spec.program);
            command.args(&spec.args);
            apply_common(&mut command, spec);
            Ok(command)
        }
    }

    fn profile(spec: &SandboxCommandSpec) -> String {
        let mut lines = vec![
            "(version 1)".to_string(),
            "(deny default)".to_string(),
            "(allow process*)".to_string(),
            "(allow sysctl-read)".to_string(),
            "(allow file-read-metadata)".to_string(),
            "(allow file-read* (subpath \"/bin\"))".to_string(),
            "(allow file-read* (subpath \"/usr\"))".to_string(),
            "(allow file-read* (subpath \"/System\"))".to_string(),
            "(allow file-read* (subpath \"/Library\"))".to_string(),
            "(allow file-read* (subpath \"/private/var/db/dyld\"))".to_string(),
        ];
        for path in &spec.allowed_paths {
            let escaped = escape_profile_path(path);
            lines.push(format!("(allow file-read* (subpath \"{escaped}\"))"));
            lines.push(format!("(allow file-write* (subpath \"{escaped}\"))"));
        }
        if spec.network == NetworkPolicy::Allow {
            lines.push("(allow network*)".to_string());
        }
        lines.join("\n")
    }

    fn escape_profile_path(path: &Path) -> String {
        path.to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;

    pub struct BwrapSandboxAdapter;

    impl SandboxAdapter for BwrapSandboxAdapter {
        fn availability(&self) -> SandboxAdapterAvailability {
            if executable_available("bwrap") {
                SandboxAdapterAvailability::Available { name: "bwrap" }
            } else {
                SandboxAdapterAvailability::Unavailable {
                    reason: "bwrap is not available on PATH".to_string(),
                }
            }
        }

        fn wrap(&self, spec: &SandboxCommandSpec) -> Result<Command, String> {
            if !self.availability().is_available() {
                return Err("bwrap is not available".to_string());
            }

            let mut command = Command::new("bwrap");
            command.args(["--die-with-parent", "--proc", "/proc", "--dev", "/dev"]);
            for path in ["/bin", "/usr", "/lib", "/lib64", "/etc"] {
                if Path::new(path).exists() {
                    command.args(["--ro-bind", path, path]);
                }
            }
            command.args(["--tmpfs", "/tmp"]);
            for path in &spec.allowed_paths {
                let path_str = path.to_string_lossy().to_string();
                command.args(["--bind", &path_str, &path_str]);
            }
            if spec.network == NetworkPolicy::Deny {
                command.arg("--unshare-net");
            }
            command.arg("--").arg(&spec.program).args(&spec.args);
            apply_common(&mut command, spec);
            Ok(command)
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
mod unsupported {
    use super::*;

    pub struct UnsupportedSandboxAdapter;

    impl SandboxAdapter for UnsupportedSandboxAdapter {
        fn availability(&self) -> SandboxAdapterAvailability {
            SandboxAdapterAvailability::Unavailable {
                reason: "sandboxing is not supported on this platform".to_string(),
            }
        }

        fn wrap(&self, _spec: &SandboxCommandSpec) -> Result<Command, String> {
            Err("sandboxing is not supported on this platform".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn executable_available_reports_missing_binary() {
        assert!(!executable_available(
            "definitely-not-a-real-rust-claude-binary"
        ));
    }
}
