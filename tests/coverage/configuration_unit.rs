use mae::app::configuration::{
    ApplicationSettings, DatabaseSettings, Environment, GraphDatabaseSettings, Settings,
    validate_production_config
};
use mae::testing::must::must_eq;
use secrecy::SecretString;

fn sample_settings(
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
            hmac_secret: SecretString::from("hmac_test".to_string()),
            cors_allowed_origin: "localhost".into()
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
fn environment_parses_known_labels() {
    must_eq(Environment::try_from("production".to_string()).expect("prod").as_str(), "production");
    must_eq(Environment::try_from("staging".to_string()).expect("staging").as_str(), "staging");
    must_eq(Environment::try_from("local".to_string()).expect("local").as_str(), "local");
    assert!(Environment::try_from("not-a-real-env".to_string()).is_err());
}

#[test]
fn validate_production_config_rejects_localhost_db_host() {
    let settings =
        sample_settings("localhost", "real-password", "neo4j.prod.internal", "neo4j-pass");
    let err = validate_production_config(&settings).unwrap_err().to_string();
    must_eq(err.contains("database.host"), true);
}

#[test]
fn database_settings_connect_options_use_host_and_port() {
    let settings = DatabaseSettings {
        username: "app".to_string(),
        password: SecretString::from("secret".to_string()),
        host: "db.internal".to_string(),
        port: 5433,
        database_name: "mae_test".to_string(),
        require_ssl: false
    };

    let options = settings.connect_options();
    must_eq(options.get_host(), "db.internal");
    must_eq(options.get_port(), 5433);
}

#[test]
fn validate_production_config_accepts_overridden_values() {
    let settings = sample_settings(
        "db.production.internal",
        "strong-production-password",
        "neo4j.production.internal",
        "strong-neo4j-password"
    );
    assert!(validate_production_config(&settings).is_ok());
}
