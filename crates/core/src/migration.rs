use serde_json::Value;
use std::path::Path;

pub trait Migration: Send + Sync {
    fn version(&self) -> u32;
    fn description(&self) -> &str;
    fn migrate(&self, config: &mut Value) -> Result<(), MigrationError>;
}

pub struct MigrationRunner {
    migrations: Vec<Box<dyn Migration>>,
}

#[derive(Debug, thiserror::Error)]
pub enum MigrationError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("migration {version} failed: {message}")]
    Failed { version: u32, message: String },
}

impl MigrationRunner {
    pub fn new() -> Self {
        Self {
            migrations: Vec::new(),
        }
    }

    pub fn with_default_migrations() -> Self {
        let mut runner = Self::new();
        runner.register(Box::new(BaselineMigration));
        runner.register(Box::new(ModelRenameMigration));
        runner
    }

    pub fn register(&mut self, migration: Box<dyn Migration>) {
        self.migrations.push(migration);
        self.migrations.sort_by_key(|migration| migration.version());
    }

    pub fn latest_version(&self) -> u32 {
        self.migrations
            .iter()
            .map(|migration| migration.version())
            .max()
            .unwrap_or(0)
    }

    pub fn run_pending(&self, config_path: &Path) -> Result<(), MigrationError> {
        if !config_path.exists() {
            return Ok(());
        }

        let content = std::fs::read_to_string(config_path)?;
        let mut config: Value = serde_json::from_str(&content)?;
        let current_version = migration_version(&config);
        let mut applied_version = current_version;

        for migration in self
            .migrations
            .iter()
            .filter(|migration| migration.version() > current_version)
        {
            migration
                .migrate(&mut config)
                .map_err(|err| MigrationError::Failed {
                    version: migration.version(),
                    message: err.to_string(),
                })?;
            applied_version = migration.version();
        }

        if applied_version != current_version {
            set_migration_version(&mut config, applied_version);
            let content = serde_json::to_string_pretty(&config)?;
            std::fs::write(config_path, content)?;
        }

        Ok(())
    }
}

impl Default for MigrationRunner {
    fn default() -> Self {
        Self::with_default_migrations()
    }
}

fn migration_version(config: &Value) -> u32 {
    config
        .get("_migration_version")
        .and_then(|value| value.as_u64())
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(0)
}

fn set_migration_version(config: &mut Value, version: u32) {
    if !config.is_object() {
        *config = Value::Object(Default::default());
    }
    if let Some(object) = config.as_object_mut() {
        object.insert("_migration_version".to_string(), Value::from(version));
    }
}

struct BaselineMigration;

impl Migration for BaselineMigration {
    fn version(&self) -> u32 {
        1
    }

    fn description(&self) -> &str {
        "baseline config migration"
    }

    fn migrate(&self, _config: &mut Value) -> Result<(), MigrationError> {
        Ok(())
    }
}

struct ModelRenameMigration;

impl Migration for ModelRenameMigration {
    fn version(&self) -> u32 {
        2
    }

    fn description(&self) -> &str {
        "rename legacy Opus 3 model setting"
    }

    fn migrate(&self, config: &mut Value) -> Result<(), MigrationError> {
        if config.get("model").and_then(|value| value.as_str()) == Some("claude-3-opus") {
            if let Some(object) = config.as_object_mut() {
                object.insert(
                    "model".to_string(),
                    Value::String("claude-opus-4-0".to_string()),
                );
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    fn temp_file(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "rust-claude-migration-{name}-{}-{}.json",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        let _ = fs::remove_file(&path);
        path
    }

    struct CountingMigration {
        version: u32,
        counter: Arc<AtomicUsize>,
    }

    impl Migration for CountingMigration {
        fn version(&self) -> u32 {
            self.version
        }

        fn description(&self) -> &str {
            "counting migration"
        }

        fn migrate(&self, _config: &mut Value) -> Result<(), MigrationError> {
            self.counter.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    struct FailingMigration;

    impl Migration for FailingMigration {
        fn version(&self) -> u32 {
            2
        }

        fn description(&self) -> &str {
            "failing migration"
        }

        fn migrate(&self, _config: &mut Value) -> Result<(), MigrationError> {
            Err(MigrationError::Failed {
                version: 2,
                message: "boom".to_string(),
            })
        }
    }

    #[test]
    fn missing_config_file_is_noop() {
        let path = temp_file("missing");
        MigrationRunner::default().run_pending(&path).unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn runs_pending_migrations_in_order() {
        let path = temp_file("pending");
        fs::write(&path, r#"{"_migration_version":1}"#).unwrap();
        let counter = Arc::new(AtomicUsize::new(0));
        let mut runner = MigrationRunner::new();
        runner.register(Box::new(CountingMigration {
            version: 3,
            counter: counter.clone(),
        }));
        runner.register(Box::new(CountingMigration {
            version: 2,
            counter: counter.clone(),
        }));
        runner.run_pending(&path).unwrap();
        let migrated: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(counter.load(Ordering::SeqCst), 2);
        assert_eq!(migration_version(&migrated), 3);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn migration_failure_stops_upgrade() {
        let path = temp_file("failure");
        fs::write(&path, r#"{"_migration_version":1}"#).unwrap();
        let counter = Arc::new(AtomicUsize::new(0));
        let mut runner = MigrationRunner::new();
        runner.register(Box::new(FailingMigration));
        runner.register(Box::new(CountingMigration {
            version: 3,
            counter: counter.clone(),
        }));
        assert!(runner.run_pending(&path).is_err());
        assert_eq!(counter.load(Ordering::SeqCst), 0);
        let migrated: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(migration_version(&migrated), 1);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn default_migration_renames_legacy_opus_model() {
        let path = temp_file("rename");
        fs::write(&path, r#"{"model":"claude-3-opus"}"#).unwrap();
        MigrationRunner::default().run_pending(&path).unwrap();
        let migrated: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(migrated["model"], "claude-opus-4-0");
        assert_eq!(migration_version(&migrated), 2);
        let _ = fs::remove_file(path);
    }
}
