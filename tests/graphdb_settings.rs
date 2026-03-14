//! Integration smoke test for GraphDatabaseSettings::connect().
//!
//! Requires MAE_TESTCONTAINERS=1 or a running Neo4j at localhost:7687.
//! Run with: cargo test --features integration-testing --test graphdb_settings

#[cfg(test)]
mod tests {
    use mae::app::configuration::GraphDatabaseSettings;
    use secrecy::SecretString;

    #[tokio::test]
    #[cfg_attr(not(feature = "integration-testing"), ignore)]
    async fn graphdb_settings_connect_returns_live_graph() {
        // Use testcontainers if available, else expect localhost:7687
        let (host, port) = if std::env::var("MAE_TESTCONTAINERS")
            .map(|v| v == "1")
            .unwrap_or(false)
        {
            let (url, bolt_port) = mae::testing::container::neo4j::get_neo4j_bolt()
                .await
                .expect("neo4j container");
            let host = url
                .trim_start_matches("bolt://")
                .rsplit_once(':')
                .map(|(h, _)| h.to_string())
                .unwrap_or_else(|| "localhost".to_string());
            (host, bolt_port)
        } else {
            ("localhost".to_string(), 7687u16)
        };

        let settings = GraphDatabaseSettings {
            host,
            port,
            username: "neo4j".to_string(),
            password: SecretString::new("neo4j".to_string().into()),
        };

        let graph = settings.connect().await.expect("should connect to neo4j");

        // Smoke test: run a simple Cypher query
        let mut result = graph
            .execute(neo4rs::query("RETURN 1 AS n"))
            .await
            .expect("query should execute");

        let row = result.next().await.expect("stream ok").expect("one row");
        let n: i64 = row.get("n").expect("field n");
        assert_eq!(n, 1);
    }
}
