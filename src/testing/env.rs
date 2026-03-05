use std::env;
use std::fmt::Write;
use std::sync::OnceLock;

use url::Url;

use super::must::MustExpect;

static CONFIG: OnceLock<DotEnv,> = OnceLock::new();

#[derive(Debug,)]
pub struct DotEnv {
    // TODO: This is rediculous, we should just build a config from yaml and add it to our ctx
    // --------------------------------------------------------------------- */
    // Migration paths                                                       */
    // ---------------------------------------------------------------------
    pub admin_migrations_path: String,
    pub app_migrations_path: String,

    // --------------------------------------------------------------------- */
    // Database identity / networking                                        */
    // ---------------------------------------------------------------------
    pub db_host: String,
    pub _db_port: u16,
    pub app_db_name: String,

    // --------------------------------------------------------------------- */
    // Roles / credentials                                                   */
    // ---------------------------------------------------------------------
    pub superuser: String,
    pub superuser_pwd: String,

    pub migrator_user: String,
    pub migrator_pwd: String,

    pub app_user: String,
    pub app_user_pwd: String,

    pub table_provisioner_user: String,
    pub table_provisioner_pwd: String,

    // --------------------------------------------------------------------- */
    // Connection strings                                                    */
    // ---------------------------------------------------------------------
    pub search_path: String,

    pub _super_database_url: String,
    pub _migrator_database_url: String,
    pub _app_database_url: String,
    pub _table_creator_database_url: String,

    /// sqlx default
    pub _database_url: String,
}

impl DotEnv {
    /// Build a Postgres DATABASE_URL using primitive env vars,
    /// overriding the port with `port`.
    ///
    /// Example:
    /// postgres://user:pwd@host:port/db?options=-csearch_path%3Dapp
    pub fn database_url_with_port(&self, port: u16,) -> String {
        build_pg_url(
            &self.migrator_user,
            &self.migrator_pwd,
            &self.db_host,
            port,
            &self.app_db_name,
            Some(&self.search_path,),
        )
    }

    /// Same builder for the app runtime user.
    pub fn app_database_url_with_port(&self, port: u16,) -> String {
        build_pg_url(
            &self.app_user,
            &self.app_user_pwd,
            &self.db_host,
            port,
            &self.app_db_name,
            Some(&self.search_path,),
        )
    }

    /// Same builder for the superuser.
    pub fn super_database_url_with_port(&self, port: u16,) -> String {
        build_pg_url(
            &self.superuser,
            &self.superuser_pwd,
            &self.db_host,
            port,
            &self.app_db_name,
            None,
        )
    }

    /// Same builder for the table provisioner.
    pub fn table_creator_database_url_with_port(&self, port: u16,) -> String {
        build_pg_url(
            &self.table_provisioner_user,
            &self.table_provisioner_pwd,
            &self.db_host,
            port,
            &self.app_db_name,
            Some(&self.search_path,),
        )
    }
}

fn build_pg_url(
    user: &str,
    password: &str,
    host: &str,
    port: u16,
    db_name: &str,
    search_path: Option<&str,>,
) -> String {
    let mut url = String::with_capacity(128,);

    // scheme + creds
    let _ = write!(&mut url, "postgres://{}:{}@{}:{}/{}", user, password, host, port, db_name);

    // optional query (already percent-encoded in env)
    let _ = match search_path {
        Some(v,) => write!(&mut url, "?{}", v),
        None => Ok((),),
    };
    url
}

pub fn load() -> &'static DotEnv {
    CONFIG.get_or_init(|| {
        // Load .env once (noop if missing)
        dotenvy::dotenv().ok();

        // ---------------- migration paths ----------------
        let admin_migrations_path = get("ADMIN_MIGRATIONS_PATH",);
        let app_migrations_path = get("APP_MIGRATIONS_PATH",);

        // ---------------- db identity ----------------
        let db_host = get("DB_HOST",);
        let db_port: u16 = get("DB_PORT",).parse().must_expect("DB_PORT must be a valid u16",);
        let app_db_name = get("APP_DB_NAME",);

        // ---------------- roles ----------------
        let superuser = get("SUPERUSER",);
        let superuser_pwd = get("SUPERUSER_PWD",);

        let migrator_user = get("MIGRATOR_USER",);
        let migrator_pwd = get("MIGRATOR_PWD",);

        let app_user = get("APP_USER",);
        let app_user_pwd = get("APP_USER_PWD",);

        let table_provisioner_user = get("TABLE_PROVISIONER_USER",);
        let table_provisioner_pwd = get("TABLE_PROVISIONER_PWD",);

        // ---------------- urls ----------------
        let search_path = get("SEARCH_PATH",);

        let raw = get("SUPER_DATABASE_URL",);
        let super_database_url = shellexpand::env(&raw,)
            .must_expect("SUPER_DATABASE_URL contains unknown env vars",)
            .into_owned();

        let raw = get("MIGRATOR_DATABASE_URL",);
        let migrator_database_url = shellexpand::env(&raw,)
            .must_expect("MIGRATOR_DATABASE_URL contains unknown env vars",)
            .into_owned();

        let raw = get("DATABASE_URL",);
        let database_url = shellexpand::env(&raw,)
            .must_expect("DATABASE_URL contains unknown env vars",)
            .into_owned();

        let raw = get("APP_DATABASE_URL",);
        let app_database_url = shellexpand::env(&raw,)
            .must_expect("APP_DATABASE_URL contains unknown env vars",)
            .into_owned();

        let raw = get("TABLE_CREATOR_DATABASE_URL",);
        let table_creator_database_url = shellexpand::env(&raw,)
            .must_expect("TABLE_CREATOR_DATABASE_URL contains unknown env vars",)
            .into_owned();

        // ---------------- safety guards ----------------
        assert_test_database(&super_database_url,);
        assert_test_database(&database_url,);
        assert_test_database(&migrator_database_url,);
        assert_test_database(&app_database_url,);
        assert_test_database(&table_creator_database_url,);

        DotEnv {
            admin_migrations_path,
            app_migrations_path,
            db_host,
            _db_port: db_port,
            app_db_name,
            superuser,
            superuser_pwd,
            migrator_user,
            migrator_pwd,
            app_user,
            app_user_pwd,
            table_provisioner_user,
            table_provisioner_pwd,
            search_path,
            _super_database_url: super_database_url,
            _migrator_database_url: migrator_database_url,
            _app_database_url: app_database_url,
            _table_creator_database_url: table_creator_database_url,
            _database_url: database_url,
        }
    },)
}

fn get(key: &str,) -> String {
    env::var(key,).must_expect(&format!("{key} must be set (env or .env)"),)
}

fn assert_test_database(database_url: &str,) {
    let url = Url::parse(database_url,)
        .must_expect(&format!("DATABASE_URL must be a valid URL: {}", database_url),);

    let db_name = url
        .path_segments()
        .and_then(|mut s| s.next_back(),)
        .filter(|s| !s.is_empty(),)
        .must_expect("DATABASE_URL must include a database name",);

    assert!(db_name.contains("_test"), "Refusing to run against non-test database: '{db_name}'");
}
