use std::sync::Once;

use anyhow::{Context, Result, anyhow};
use rusqlite::Connection;
use rusqlite::OptionalExtension;
use rusqlite::ffi::sqlite3_auto_extension;
use rusqlite::params;
use sqlite_vec::sqlite3_vec_init;

pub const WORKSPACE_DIR: &str = ".memento";
pub const INDEX_FILE: &str = ".memento/index.db";
pub const USER_DIR: &str = ".memento/user";
pub const AGENT_DIR: &str = ".memento/agent";

const BASE_SCHEMA_SQL: &str = r#"
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS workspace_meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
) STRICT;

INSERT INTO workspace_meta(key, value)
VALUES
    ('schema_version', '6'),
    ('vector_extension', 'sqlite-vec')
ON CONFLICT(key) DO UPDATE SET value = excluded.value;

CREATE TABLE IF NOT EXISTS items (
    id INTEGER PRIMARY KEY,
    uri TEXT NOT NULL UNIQUE,
    namespace TEXT NOT NULL,
    kind TEXT NOT NULL,
    source_path TEXT UNIQUE,
    file_size_bytes INTEGER,
    modified_at TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
) STRICT;

CREATE TABLE IF NOT EXISTS content_layers (
    id INTEGER PRIMARY KEY,
    item_id INTEGER NOT NULL,
    layer TEXT NOT NULL,
    storage_kind TEXT NOT NULL,
    body TEXT,
    checksum TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(item_id) REFERENCES items(id) ON DELETE CASCADE,
    UNIQUE(item_id, layer)
) STRICT;

CREATE TABLE IF NOT EXISTS vector_spans (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    item_id INTEGER NOT NULL,
    layer TEXT NOT NULL,
    scope TEXT NOT NULL,
    start_line INTEGER,
    end_line INTEGER,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(item_id) REFERENCES items(id) ON DELETE CASCADE,
    UNIQUE(item_id, layer, scope, start_line, end_line)
) STRICT;

CREATE INDEX IF NOT EXISTS idx_items_namespace_uri ON items(namespace, uri);
CREATE INDEX IF NOT EXISTS idx_vector_spans_item_id ON vector_spans(item_id);

"#;

static SQLITE_VEC_INIT: Once = Once::new();

#[derive(Debug, Clone)]
pub struct NewItem<'a> {
    pub uri: &'a str,
    pub namespace: &'a str,
    pub kind: &'a str,
    pub source_path: Option<&'a str>,
    pub file_size_bytes: Option<i64>,
    pub modified_at: Option<&'a str>,
}

#[derive(Debug, Clone)]
pub struct ItemRecord {
    pub id: i64,
    pub uri: String,
    pub namespace: String,
    pub kind: String,
    pub source_path: Option<String>,
    pub file_size_bytes: Option<i64>,
    pub modified_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct NewContentLayer<'a> {
    pub item_id: i64,
    pub layer: &'a str,
    pub storage_kind: &'a str,
    pub body: Option<&'a str>,
    pub checksum: Option<&'a str>,
}

#[derive(Debug, Clone)]
pub struct ContentLayerRecord {
    pub layer: String,
    pub storage_kind: String,
}

#[derive(Debug, Clone)]
pub struct NewVectorSpan<'a> {
    pub item_id: i64,
    pub layer: &'a str,
    pub scope: &'a str,
    pub start_line: Option<i64>,
    pub end_line: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct VectorSpanRecord {
    pub id: i64,
}

#[derive(Debug, Clone)]
pub struct SearchResultRecord {
    pub uri: String,
    pub source_path: Option<String>,
    pub namespace: String,
    pub kind: String,
    pub file_size_bytes: Option<i64>,
    pub modified_at: Option<String>,
    pub distance: f64,
    pub layer: String,
    pub scope: String,
    pub start_line: Option<i64>,
    pub end_line: Option<i64>,
}

pub struct WorkspaceRepository {
    connection: Connection,
}

impl WorkspaceRepository {
    pub fn open(path: &str) -> Result<Self> {
        register_sqlite_vec();

        let connection =
            Connection::open(path).with_context(|| format!("failed to open database `{path}`"))?;

        Ok(Self { connection })
    }

    pub fn initialize_schema(&self) -> Result<()> {
        self.connection
            .execute_batch(BASE_SCHEMA_SQL)
            .context("failed to initialize workspace schema")
    }

    pub fn initialize_vector_schema(&self, embedding_dimension: usize) -> Result<()> {
        let sql = format!(
            r#"
            DROP TABLE IF EXISTS vector_embeddings;
            CREATE VIRTUAL TABLE vector_embeddings USING vec0(
                span_id INTEGER PRIMARY KEY,
                embedding FLOAT[{embedding_dimension}]
            );
            "#
        );

        self.connection
            .execute_batch(&sql)
            .context("failed to initialize vector schema")
    }

    pub fn set_workspace_meta(&self, key: &str, value: &str) -> Result<()> {
        self.connection
            .execute(
                r#"
                INSERT INTO workspace_meta (key, value)
                VALUES (?1, ?2)
                ON CONFLICT(key) DO UPDATE SET value = excluded.value
                "#,
                params![key, value],
            )
            .with_context(|| format!("failed to write workspace metadata `{key}`"))?;

        Ok(())
    }

    pub fn get_workspace_meta(&self, key: &str) -> Result<Option<String>> {
        self.connection
            .query_row(
                "SELECT value FROM workspace_meta WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()
            .with_context(|| format!("failed to read workspace metadata `{key}`"))
    }

    pub fn ensure_vector_schema_matches(&self, embedding_dimension: usize) -> Result<()> {
        let stored_dimension = self.get_workspace_meta("embedding_dimension")?;

        if stored_dimension.as_deref() != Some(&embedding_dimension.to_string()) {
            return Err(anyhow!(
                "workspace embedding dimension does not match initialized vector schema"
            ));
        }

        if !self.vector_embeddings_table_exists()? {
            self.initialize_vector_schema(embedding_dimension)?;
        }

        Ok(())
    }

    pub fn upsert_item(&self, item: &NewItem<'_>) -> Result<()> {
        self.connection
            .execute(
                r#"
                INSERT INTO items (uri, namespace, kind, source_path, file_size_bytes, modified_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                ON CONFLICT(uri) DO UPDATE SET
                    namespace = excluded.namespace,
                    kind = excluded.kind,
                    source_path = excluded.source_path,
                    file_size_bytes = excluded.file_size_bytes,
                    modified_at = excluded.modified_at,
                    updated_at = CURRENT_TIMESTAMP
                "#,
                params![
                    item.uri,
                    item.namespace,
                    item.kind,
                    item.source_path,
                    item.file_size_bytes,
                    item.modified_at,
                ],
            )
            .context("failed to upsert item")?;

        Ok(())
    }

    pub fn replace_content_layer(&self, layer: &NewContentLayer<'_>) -> Result<()> {
        self.connection
            .execute(
                r#"
                INSERT INTO content_layers (item_id, layer, storage_kind, body, checksum)
                VALUES (?1, ?2, ?3, ?4, ?5)
                ON CONFLICT(item_id, layer) DO UPDATE SET
                    storage_kind = excluded.storage_kind,
                    body = excluded.body,
                    checksum = excluded.checksum,
                    updated_at = CURRENT_TIMESTAMP
                "#,
                params![
                    layer.item_id,
                    layer.layer,
                    layer.storage_kind,
                    layer.body,
                    layer.checksum,
                ],
            )
            .context("failed to replace content layer")?;

        Ok(())
    }

    pub fn list_content_layers(&self, item_id: i64) -> Result<Vec<ContentLayerRecord>> {
        let mut statement = self
            .connection
            .prepare(
                r#"
                SELECT layer, storage_kind
                FROM content_layers
                WHERE item_id = ?1
                ORDER BY layer ASC
                "#,
            )
            .context("failed to prepare content layer query")?;

        let rows = statement
            .query_map(params![item_id], |row| {
                Ok(ContentLayerRecord {
                    layer: row.get(0)?,
                    storage_kind: row.get(1)?,
                })
            })
            .context("failed to query content layers")?;

        rows.collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to read content layers")
    }

    pub fn get_item_by_source_path(&self, source_path: &str) -> Result<Option<ItemRecord>> {
        self.connection
            .query_row(
                r#"
                SELECT id, uri, namespace, kind, source_path, file_size_bytes, modified_at, created_at, updated_at
                FROM items
                WHERE source_path = ?1
                "#,
                params![source_path],
                map_item_row,
            )
            .optional()
            .context("failed to query item by source path")
    }

    pub fn replace_item_spans(&self, item_id: i64, spans: &[NewVectorSpan<'_>]) -> Result<()> {
        self.clear_item_vectors(item_id)?;

        let mut statement = self
            .connection
            .prepare(
                r#"
                INSERT INTO vector_spans (item_id, layer, scope, start_line, end_line)
                VALUES (?1, ?2, ?3, ?4, ?5)
                "#,
            )
            .context("failed to prepare vector span insert")?;

        for span in spans {
            statement
                .execute(params![
                    span.item_id,
                    span.layer,
                    span.scope,
                    span.start_line,
                    span.end_line,
                ])
                .context("failed to insert vector span")?;
        }

        Ok(())
    }

    pub fn delete_item(&self, item_id: i64) -> Result<()> {
        self.clear_item_vectors(item_id)?;

        self.connection
            .execute("DELETE FROM items WHERE id = ?1", params![item_id])
            .context("failed to delete item")?;

        Ok(())
    }

    pub fn list_item_spans(&self, item_id: i64) -> Result<Vec<VectorSpanRecord>> {
        let mut statement = self
            .connection
            .prepare(
                r#"
                SELECT id
                FROM vector_spans
                WHERE item_id = ?1
                ORDER BY start_line ASC, end_line ASC, id ASC
                "#,
            )
            .context("failed to prepare item span query")?;

        let rows = statement
            .query_map(params![item_id], |row| {
                Ok(VectorSpanRecord { id: row.get(0)? })
            })
            .context("failed to query item spans")?;

        rows.collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to read item spans")
    }

    pub fn replace_vector_embedding(&self, span_id: i64, embedding: &[u8]) -> Result<()> {
        self.connection
            .execute(
                "DELETE FROM vector_embeddings WHERE span_id = ?1",
                params![span_id],
            )
            .context("failed to delete existing vector embedding")?;

        self.connection
            .execute(
                r#"
                INSERT INTO vector_embeddings (span_id, embedding)
                VALUES (?1, ?2)
                "#,
                params![span_id, embedding],
            )
            .context("failed to insert vector embedding")?;

        Ok(())
    }

    pub fn search_similar_items(
        &self,
        query_embedding: &[u8],
        limit: i64,
    ) -> Result<Vec<SearchResultRecord>> {
        let mut statement = self
            .connection
            .prepare(
                r#"
                WITH knn_matches AS (
                    SELECT span_id, distance
                    FROM vector_embeddings
                    WHERE embedding MATCH ?1
                      AND k = ?2
                )
                SELECT items.uri, items.source_path, items.namespace, items.kind,
                       items.file_size_bytes, items.modified_at, knn_matches.distance,
                       vector_spans.layer, vector_spans.scope, vector_spans.start_line,
                       vector_spans.end_line
                FROM knn_matches
                INNER JOIN vector_spans ON vector_spans.id = knn_matches.span_id
                INNER JOIN items ON items.id = vector_spans.item_id
                ORDER BY knn_matches.distance ASC
                "#,
            )
            .context("failed to prepare vector search query")?;

        let rows = statement
            .query_map(params![query_embedding, limit], |row| {
                Ok(SearchResultRecord {
                    uri: row.get(0)?,
                    source_path: row.get(1)?,
                    namespace: row.get(2)?,
                    kind: row.get(3)?,
                    file_size_bytes: row.get(4)?,
                    modified_at: row.get(5)?,
                    distance: row.get(6)?,
                    layer: row.get(7)?,
                    scope: row.get(8)?,
                    start_line: row.get(9)?,
                    end_line: row.get(10)?,
                })
            })
            .context("failed to execute vector search query")?;

        rows.collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to read vector search results")
    }

    pub fn list_items(&self) -> Result<Vec<ItemRecord>> {
        let mut statement = self
            .connection
            .prepare(
                r#"
                SELECT id, uri, namespace, kind, source_path, file_size_bytes, modified_at, created_at, updated_at
                FROM items
                ORDER BY uri ASC
                "#,
            )
            .context("failed to prepare item listing query")?;

        let rows = statement
            .query_map([], map_item_row)
            .context("failed to query items")?;

        rows.collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to read items")
    }

    pub fn item_count(&self) -> Result<i64> {
        self.connection
            .query_row("SELECT COUNT(*) FROM items", [], |row| row.get(0))
            .context("failed to count items")
    }

    pub fn get_item_by_uri(&self, uri: &str) -> Result<Option<ItemRecord>> {
        self.connection
            .query_row(
                r#"
                SELECT id, uri, namespace, kind, source_path, file_size_bytes, modified_at, created_at, updated_at
                FROM items
                WHERE uri = ?1
                "#,
                params![uri],
                map_item_row,
            )
            .optional()
            .context("failed to query item by URI")
    }

    fn clear_item_vectors(&self, item_id: i64) -> Result<()> {
        let span_ids = self
            .list_item_spans(item_id)?
            .into_iter()
            .map(|span| span.id)
            .collect::<Vec<_>>();

        for span_id in span_ids {
            self.connection
                .execute(
                    "DELETE FROM vector_embeddings WHERE span_id = ?1",
                    params![span_id],
                )
                .context("failed to delete item vector embeddings")?;
        }

        self.connection
            .execute(
                "DELETE FROM vector_spans WHERE item_id = ?1",
                params![item_id],
            )
            .context("failed to clear existing item vector spans")?;

        Ok(())
    }

    fn vector_embeddings_table_exists(&self) -> Result<bool> {
        self.connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'vector_embeddings'",
                [],
                |row| {
                    let count: i64 = row.get(0)?;
                    Ok(count > 0)
                },
            )
            .context("failed to inspect vector_embeddings table")
    }
}

fn map_item_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ItemRecord> {
    Ok(ItemRecord {
        id: row.get(0)?,
        uri: row.get(1)?,
        namespace: row.get(2)?,
        kind: row.get(3)?,
        source_path: row.get(4)?,
        file_size_bytes: row.get(5)?,
        modified_at: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

fn register_sqlite_vec() {
    SQLITE_VEC_INIT.call_once(|| unsafe {
        sqlite3_auto_extension(Some(std::mem::transmute::<
            *const (),
            unsafe extern "C" fn(
                *mut rusqlite::ffi::sqlite3,
                *mut *mut i8,
                *const rusqlite::ffi::sqlite3_api_routines,
            ) -> i32,
        >(sqlite3_vec_init as *const ())));
    });
}
