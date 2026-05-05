#![allow(dead_code)]

use std::{ops::Deref, time::Duration};

use pgsense_rs::{config::Secret, pipeline::config::DatabaseConfig};
use testcontainers_modules::{
    postgres::Postgres,
    testcontainers::{ContainerAsync, ImageExt, runners::AsyncRunner},
};

pub const CREATE_TABLE: &str = "CREATE TABLE test_data (id SERIAL PRIMARY KEY, col_a TEXT NOT NULL, col_b TEXT)";
pub const PUBLICATION: &str = "pgsense_pub";

/// Wrapper around `tokio_postgres::Client` that aborts the connection driver
/// task on drop, preventing leaked background tasks in tests.
pub struct PgClient {
    client: tokio_postgres::Client,
    conn_handle: tokio::task::JoinHandle<()>,
}

impl Deref for PgClient {
    type Target = tokio_postgres::Client;
    fn deref(&self) -> &Self::Target {
        &self.client
    }
}

impl Drop for PgClient {
    fn drop(&mut self) {
        self.conn_handle.abort();
    }
}

pub struct PgContainer {
    pub host: String,
    pub port: u16,
    _container: ContainerAsync<Postgres>,
}

impl PgContainer {
    /// Start a plain PG 16 container (no WAL config).
    pub async fn start() -> Self {
        Self::start_with_cmd(&[]).await
    }

    /// Start a PG 16 container with logical replication enabled.
    pub async fn start_with_wal() -> Self {
        Self::start_with_cmd(&[
            "postgres",
            "-c",
            "wal_level=logical",
            "-c",
            "max_replication_slots=4",
            "-c",
            "max_wal_senders=4",
        ])
        .await
    }

    async fn start_with_cmd(cmd: &[&str]) -> Self {
        let mut builder = Postgres::default().with_tag("16-alpine");
        if !cmd.is_empty() {
            builder = builder.with_cmd(cmd.to_vec());
        }
        let container = builder
            .start()
            .await
            .expect("failed to start postgres container");
        let host = container.get_host().await.expect("get host").to_string();
        let port = container.get_host_port_ipv4(5432).await.expect("get port");
        Self {
            host,
            port,
            _container: container,
        }
    }

    /// Connect to a database on this container with retries.
    pub async fn connect(&self, dbname: &str) -> PgClient {
        let conn_str = format!(
            "host={} port={} user=postgres password=postgres dbname={dbname}",
            self.host, self.port
        );
        pg_connect(&conn_str).await
    }

    /// Create test_data table + publication on the default `postgres` database.
    pub async fn setup_database(&self) -> PgClient {
        let client = self.connect("postgres").await;
        client
            .execute(CREATE_TABLE, &[])
            .await
            .expect("create table");
        client
            .execute(&format!("CREATE PUBLICATION {PUBLICATION} FOR TABLE test_data"), &[])
            .await
            .expect("create publication");
        client
    }

    /// Create a new database, then create test_data table + publication in it.
    pub async fn create_database(&self, dbname: &str) -> PgClient {
        let admin = self.connect("postgres").await;
        admin
            .execute(&format!("CREATE DATABASE {dbname}"), &[])
            .await
            .expect("create database");
        drop(admin);

        let client = self.connect(dbname).await;
        client
            .execute(CREATE_TABLE, &[])
            .await
            .expect("create table");
        client
            .execute(&format!("CREATE PUBLICATION {PUBLICATION} FOR TABLE test_data"), &[])
            .await
            .expect("create publication");
        client
    }

    /// Build a `DatabaseConfig` pointing at this container.
    pub fn db_config(&self, dbname: &str) -> DatabaseConfig {
        DatabaseConfig {
            host: self.host.clone(),
            port: self.port,
            dbname: dbname.to_string(),
            username: "postgres".to_string(),
            password: Some(Secret::from("postgres")),
            publication: PUBLICATION.to_string(),
            ..Default::default()
        }
    }
}

/// Connect to postgres with retries (container may not be ready immediately).
async fn pg_connect(conn_str: &str) -> PgClient {
    let max_attempts = 10;
    for attempt in 1..=max_attempts {
        match tokio_postgres::connect(conn_str, tokio_postgres::NoTls).await {
            Ok((client, connection)) => {
                let conn_handle = tokio::spawn(async move {
                    if let Err(e) = connection.await {
                        eprintln!("pg connection error: {e}");
                    }
                });
                return PgClient { client, conn_handle };
            },
            Err(e) => {
                if attempt == max_attempts {
                    panic!("failed to connect after {max_attempts} attempts: {e}");
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            },
        }
    }
    unreachable!()
}
