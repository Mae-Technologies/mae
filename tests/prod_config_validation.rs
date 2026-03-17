//! Production configuration validation tests.
//!
//! All forbidden-default values are hardcoded here (not loaded from base.yaml)
//! so the tests are stable regardless of what base.yaml contains.

use mae::app::configuration::{
    ApplicationSettings, DatabaseSettings, Environment, GraphDatabaseSettings, Settings,
    validate_production_config
};
use secrecy::SecretString;

/// Forbidden dev-default values that must never appear in production.
/// These are the exact strings checked by `validate_production_config`.
const FORBIDDEN_DB_PASSWORD: &str = "secret";
const FORBIDDEN_DB_HOST_LOCALHOST: &str = "localhost";
const FORBIDDEN_DB_HOST_ZERO: &str = "0.0.0.0";
const FORBIDDEN_GRAPHDB_PASSWORD: &str = "testpassword";
const FORBIDDEN_GRAPHDB_HOST: &str = "localhost";

/// Build a [`Settings<()>`] with the given DB and graphdb host/password.
/// All other fields are inert placeholders.
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
            base_url: "http://placeholder:8080".to_string(),
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
fn production_with_all_dev_defaults_returns_err() {
    let settings = make_settings(
        FORBIDDEN_DB_HOST_LOCALHOST,
        FORBIDDEN_DB_PASSWORD,
        FORBIDDEN_GRAPHDB_HOST,
        FORBIDDEN_GRAPHDB_PASSWORD
    );
    let result = validate_production_config(&settings);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("graphdb.password"), "should flag graphdb.password");
    assert!(err.contains("database.password"), "should flag database.password");
    assert!(err.contains("graphdb.host"), "should flag graphdb.host");
    assert!(err.contains("database.host"), "should flag database.host");
}

#[test]
fn production_with_overridden_values_returns_ok() {
    let settings = make_settings(
        "db.production.internal",
        "strong-production-password",
        "neo4j.production.internal",
        "strong-neo4j-password"
    );
    assert!(validate_production_config(&settings).is_ok());
}

#[test]
fn catches_graphdb_password_default() {
    let settings = make_settings(
        "db.prod.internal",
        "real-password",
        "neo4j.prod.internal",
        FORBIDDEN_GRAPHDB_PASSWORD
    );
    let err = validate_production_config(&settings).unwrap_err().to_string();
    assert!(err.contains("graphdb.password"));
    assert!(!err.contains("database.password"));
}

#[test]
fn catches_db_password_default() {
    let settings = make_settings(
        "db.prod.internal",
        FORBIDDEN_DB_PASSWORD,
        "neo4j.prod.internal",
        "real-neo4j-password"
    );
    let err = validate_production_config(&settings).unwrap_err().to_string();
    assert!(err.contains("database.password"));
}

#[test]
fn catches_db_host_zero() {
    let settings = make_settings(
        FORBIDDEN_DB_HOST_ZERO,
        "real-password",
        "neo4j.prod.internal",
        "real-neo4j-password"
    );
    let err = validate_production_config(&settings).unwrap_err().to_string();
    assert!(err.contains("database.host"));
}

#[test]
fn catches_graphdb_host_localhost() {
    let settings = make_settings(
        "db.prod.internal",
        "real-password",
        FORBIDDEN_GRAPHDB_HOST,
        "real-neo4j-password"
    );
    let err = validate_production_config(&settings).unwrap_err().to_string();
    assert!(err.contains("graphdb.host"));
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
    assert!(Environment::try_from("invalid".to_string()).is_err());
}
