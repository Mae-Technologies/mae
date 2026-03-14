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
        .add_source(config::File::from(configuration_directory.join(environment_filename)))
        .add_source(config::Environment::with_prefix("APP").prefix_separator("_").separator("__"))
        .build()
        .with_context(|| "failed to build configurations")?;
    settings.try_deserialize::<Settings<S>>().with_context(|| "failed to deserialize configuration")
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
    pub database_admin: Option<DatabaseAdminSettings>
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
    Production,
    Test
}

impl Environment {
    pub fn as_str(&self) -> &'static str {
        match self {
            Environment::Local => "local",
            Environment::Production => "production",
            Environment::Test => "test"
        }
    }
}

impl TryFrom<String> for Environment {
    type Error = anyhow::Error;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        match s.to_lowercase().as_str() {
            "local" => Ok(Self::Local),
            "production" => Ok(Self::Production),
            "test" => Ok(Self::Test),
            other => Err(anyhow!("{} is not a supported environment", other))
        }
    }
}
