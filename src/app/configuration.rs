use anyhow::{Context, Result, anyhow};
use secrecy::{ExposeSecret, SecretString};
use serde_aux::field_attributes::deserialize_number_from_string;
use sqlx::postgres::{PgConnectOptions, PgPool, PgPoolOptions, PgSslMode};

// get the configuration from the YAML files located in 'configuration' directory
pub fn get_configuration<S: for<'a> serde::Deserialize<'a>>() -> Result<Settings<S>> {
    let base_path =
        std::env::current_dir().with_context(|| "Failed to determine the current directory")?;
    let configuration_directory = base_path.join("configuration");

    let environment: Environment = std::env::var("APP_ENVIRONMENT")
        .unwrap_or_else(|_| "local".into())
        .try_into()
        .with_context(|| "Failed to parse APP_ENVIRONMENT")?;

    let environment_filename = format!("{}.yaml", environment.as_str());

    let settings = config::Config::builder()
        .add_source(config::File::from(configuration_directory.join("base.yaml")))
        .add_source(
            config::File::from(configuration_directory.join(environment_filename)).required(false)
        )
        .add_source(config::Environment::with_prefix("APP").prefix_separator("_").separator("__"))
        .build()
        .with_context(|| "failed to build configurations")?;
    let settings = settings
        .try_deserialize::<Settings<S>>()
        .with_context(|| "failed to deserialize configuration")?;

    if matches!(environment, Environment::Production | Environment::Staging) {
        validate_production_config(&settings)?;
    }

    Ok(settings)
}

/// Known dev-default values that must never appear in production / staging.
///
/// Each tuple is `(dotted field path, actual value, forbidden default)`.
/// The check is case-sensitive and exact-match.
fn validate_production_config<S>(settings: &Settings<S>) -> Result<()> {
    let checks: Vec<(&str, &str, &str)> = vec![
        ("graphdb.password", settings.graphdb.password.expose_secret(), "testpassword"),
        ("graphdb.host", &settings.graphdb.host, "localhost"),
        ("database.password", settings.database.password.expose_secret(), "secret"),
        ("database.host", &settings.database.host, "localhost"),
        ("database.host", &settings.database.host, "0.0.0.0"),
    ];

    let mut errors: Vec<String> = Vec::new();

    for (field, actual, forbidden) in &checks {
        if *actual == *forbidden {
            errors.push(format!(
                "[production] configuration field `{field}` must be explicitly set \
                 — fallback defaults are not permitted in production"
            ));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(anyhow!("Production configuration validation failed:\n{}", errors.join("\n")))
    }
}

// DEFAULT CONFIGS
//

// Settings: where T is the application specific settings, gathered under 'custom' in the
// configuration YAML files
#[derive(serde::Deserialize, Clone)]
pub struct Settings<T> {
    pub database: DatabaseSettings,
    pub application: ApplicationSettings,
    pub redis_uri: SecretString,
    pub custom: T,
    #[serde(default)]
    pub database_admin: Option<DatabaseAdminSettings>,
    pub graphdb: GraphDatabaseSettings
}

/// Admin / provisioning credentials used by the mae testing framework.
///
/// When present in `configuration/base.yaml` under the `database_admin` key,
/// these values replace the old `.env` / dotenvy approach entirely.  Every
/// field carries a sensible default so services only need to override what
/// differs from the standard mae-postgres layout.
#[derive(serde::Deserialize, Clone, Debug)]
pub struct DatabaseAdminSettings {
    #[serde(default = "default_admin_migrations_path")]
    pub admin_migrations_path: String,
    #[serde(default = "default_app_migrations_path")]
    pub app_migrations_path: String,
    #[serde(default = "default_superuser")]
    pub superuser: String,
    #[serde(default = "default_superuser_pwd")]
    pub superuser_pwd: String,
    #[serde(default = "default_migrator_user")]
    pub migrator_user: String,
    #[serde(default = "default_migrator_pwd")]
    pub migrator_pwd: String,
    #[serde(default = "default_app_user")]
    pub app_user: String,
    #[serde(default = "default_app_user_pwd")]
    pub app_user_pwd: String,
    #[serde(default = "default_table_provisioner_user")]
    pub table_provisioner_user: String,
    #[serde(default = "default_table_provisioner_pwd")]
    pub table_provisioner_pwd: String,
    #[serde(default = "default_search_path")]
    pub search_path: String
}

fn default_admin_migrations_path() -> String {
    "admin_migrations".into()
}
fn default_app_migrations_path() -> String {
    "migrations".into()
}
fn default_superuser() -> String {
    "postgres".into()
}
fn default_superuser_pwd() -> String {
    "password".into()
}
fn default_migrator_user() -> String {
    "db_migrator".into()
}
fn default_migrator_pwd() -> String {
    "migrator_secret".into()
}
fn default_app_user() -> String {
    "app".into()
}
fn default_app_user_pwd() -> String {
    "secret".into()
}
fn default_table_provisioner_user() -> String {
    "table_provisioner".into()
}
fn default_table_provisioner_pwd() -> String {
    "provisioner_secret".into()
}
fn default_search_path() -> String {
    "options=-csearch_path%3Dapp".into()
}
// DATABASE SETTINGS
#[derive(serde::Deserialize, Clone)]
pub struct DatabaseSettings {
    pub username: String,
    pub password: SecretString,
    pub host: String,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,
    pub database_name: String,
    pub require_ssl: bool
}

impl DatabaseSettings {
    pub fn connect_options(&self) -> PgConnectOptions {
        let ssl_mode = if self.require_ssl { PgSslMode::Require } else { PgSslMode::Prefer };

        PgConnectOptions::new()
            .host(&self.host)
            .port(self.port)
            .password(self.password.expose_secret())
            .username(&self.username)
            .ssl_mode(ssl_mode)
            .database(&self.database_name)
    }

    pub fn get_connection_pool(&self) -> PgPool {
        PgPoolOptions::new().connect_lazy_with(self.connect_options())
    }
}

// GRAPH DATABASE SETTINGS

/// Connection settings for a Neo4j graph database.
///
/// Deserialised from the `graphdb` key in `base.yaml`:
///
/// ```yaml
/// graphdb:
///   host: "localhost"
///   port: 7687
///   username: "neo4j"
///   password: "secret"
/// ```
#[derive(serde::Deserialize, Clone)]
pub struct GraphDatabaseSettings {
    pub host: String,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,
    pub username: String,
    pub password: SecretString
}

impl GraphDatabaseSettings {
    /// Connect to Neo4j and return a live [`neo4rs::Graph`] handle.
    ///
    /// # Errors
    /// Returns an error if the Bolt connection cannot be established.
    pub async fn connect(&self) -> anyhow::Result<neo4rs::Graph> {
        let bolt_url = format!("bolt://{}:{}", self.host, self.port);
        neo4rs::Graph::new(&bolt_url, self.username.clone(), self.password.expose_secret())
            .await
            .with_context(|| format!("failed to connect to Neo4j at {}", bolt_url))
    }

    /// Load [`GraphDatabaseSettings`] from the `graphdb` key in the YAML config files.
    ///
    /// Reads `configuration/base.yaml` and the environment-specific override
    /// (`APP_ENVIRONMENT`, defaulting to `"test"`). Useful in tests that need
    /// credentials from config without loading the full [`Settings<T>`].
    ///
    /// # Errors
    /// Returns an error if the config files are missing or the `graphdb` key
    /// cannot be deserialised.
    pub fn from_config() -> anyhow::Result<Self> {
        let base_path = std::env::current_dir().context("failed to determine current directory")?;
        let config_dir = base_path.join("configuration");
        let env_name = std::env::var("APP_ENVIRONMENT").unwrap_or_else(|_| "test".into());
        let raw = config::Config::builder()
            .add_source(config::File::from(config_dir.join("base.yaml")))
            .add_source(config::File::from(config_dir.join(format!("{env_name}.yaml"))))
            .build()
            .context("failed to build configuration")?;
        raw.get::<Self>("graphdb").context("failed to deserialise graphdb configuration")
    }
}

// APPLICATION & ENVIRONMENT
#[derive(serde::Deserialize, Clone)]
pub struct ApplicationSettings {
    pub host: String,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,
    pub base_url: String,
    pub hmac_secret: SecretString
}

pub enum Environment {
    Local,
    Dev,
    Test,
    Staging,
    Production
}

impl Environment {
    pub fn as_str(&self) -> &'static str {
        match self {
            Environment::Local => "local",
            Environment::Dev => "dev",
            Environment::Test => "test",
            Environment::Staging => "staging",
            Environment::Production => "production"
        }
    }
}

impl TryFrom<String> for Environment {
    type Error = anyhow::Error;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        match s.to_lowercase().as_str() {
            "local" => Ok(Self::Local),
            "dev" => Ok(Self::Dev),
            "test" => Ok(Self::Test),
            "staging" => Ok(Self::Staging),
            "production" => Ok(Self::Production),
            other => Err(anyhow!("{} is not a supported environment", other))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use secrecy::SecretString;

    fn make_settings(
        db_host: &str,
        db_password: &str,
        graphdb_host: &str,
        graphdb_password: &str
    ) -> Settings<()> {
        Settings {
            database: DatabaseSettings {
                username: "app".to_string(),
                password: SecretString::from(db_password.to_string()),
                host: db_host.to_string(),
                port: 5432,
                database_name: "test_db".to_string(),
                require_ssl: false
            },
            application: ApplicationSettings {
                host: "0.0.0.0".to_string(),
                port: 8080,
                base_url: "http://localhost:8080".to_string(),
                hmac_secret: SecretString::from("hmac_test".to_string())
            },
            redis_uri: SecretString::from("redis://127.0.0.1:6379".to_string()),
            custom: (),
            database_admin: None,
            graphdb: GraphDatabaseSettings {
                host: graphdb_host.to_string(),
                port: 7687,
                username: "neo4j".to_string(),
                password: SecretString::from(graphdb_password.to_string())
            }
        }
    }

    #[test]
    fn production_with_dev_defaults_returns_err() {
        let settings = make_settings("localhost", "secret", "localhost", "testpassword");
        let result = validate_production_config(&settings);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("graphdb.password"));
        assert!(err.contains("database.password"));
        assert!(err.contains("graphdb.host"));
        assert!(err.contains("database.host"));
    }

    #[test]
    fn production_with_overridden_values_returns_ok() {
        let settings = make_settings(
            "db.production.internal",
            "strong-production-password",
            "neo4j.production.internal",
            "strong-neo4j-password"
        );
        let result = validate_production_config(&settings);
        assert!(result.is_ok());
    }

    #[test]
    fn production_with_partial_defaults_catches_graphdb_password() {
        let settings = make_settings(
            "db.prod.internal",
            "real-password",
            "neo4j.prod.internal",
            "testpassword"
        );
        let result = validate_production_config(&settings);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("graphdb.password"));
        assert!(!err.contains("database.password"));
    }

    #[test]
    fn production_with_partial_defaults_catches_db_password() {
        let settings = make_settings(
            "db.prod.internal",
            "secret",
            "neo4j.prod.internal",
            "real-neo4j-password"
        );
        let result = validate_production_config(&settings);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("database.password"));
    }

    #[test]
    fn production_with_zero_host_returns_err() {
        let settings =
            make_settings("0.0.0.0", "real-password", "neo4j.prod.internal", "real-neo4j-password");
        let result = validate_production_config(&settings);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("database.host"));
    }

    // Local/Test environments skip validation entirely — the function
    // is only called for Production/Staging in get_configuration.
    #[test]
    fn validate_fn_itself_still_catches_defaults_regardless() {
        // The function doesn't check environment — it always validates.
        // The gating happens in get_configuration. Here we just verify
        // that dev defaults are always flagged when the function is called.
        let settings = make_settings("localhost", "secret", "localhost", "testpassword");
        assert!(validate_production_config(&settings).is_err());
    }

    #[test]
    fn environment_as_str_roundtrip() {
        assert_eq!(Environment::Local.as_str(), "local");
        assert_eq!(Environment::Dev.as_str(), "dev");
        assert_eq!(Environment::Test.as_str(), "test");
        assert_eq!(Environment::Staging.as_str(), "staging");
        assert_eq!(Environment::Production.as_str(), "production");
    }

    #[test]
    fn environment_try_from_valid() {
        let prod = Environment::try_from("Production".to_string());
        assert!(prod.is_ok());
        assert_eq!(prod.unwrap().as_str(), "production");
    }

    #[test]
    fn environment_try_from_invalid() {
        let result = Environment::try_from("invalid".to_string());
        assert!(result.is_err());
    }
}
