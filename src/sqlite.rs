use rusqlite::Connection;
use failure::{Error, SyncFailure};
use std::path::Path;
use serde_rusqlite::{from_rows, to_params_named};
use time;

pub struct Database {
    db: Connection,
}

impl Database {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, Error> {
        let db = Connection::open(path)?;
        Self::from_connection(db)
    }

    pub fn open_in_memory() -> Result<Self, Error> {
        let db = Connection::open_in_memory()?;
        Self::from_connection(db)
    }

    fn from_connection(db: Connection) -> Result<Self, Error> {
        db.execute("CREATE TABLE IF NOT EXISTS posts (
            id              INTEGER PRIMARY KEY,
            title           TEXT NOT NULL,
            url             TEXT NOT NULL,
            user            TEXT NOT NULL,
            channel         TEXT NOT NULL,
            time_created    TEXT NOT NULL
            )",
            &[]
        )?;
        db.execute("CREATE TABLE IF NOT EXISTS errors (
            id              INTEGER PRIMARY KEY,
            url             TEXT NOT NULL,
            error           TEXT NOT NULL,
            headers         TEXT NOT NULL,
            status          TEXT NOT NULL,
            time_created    TEXT NOT NULL
            )",
            &[]
        )?;

        Ok(Self { db })
    }

    pub fn add_log(&self, entry: &NewLogEntry) -> Result<(), Error> {
        let time_created = time::now().to_local().ctime().to_string();
        let params = to_params_named(entry).map_err(SyncFailure::new)?;
        let mut params = params.to_slice();
        params.push((":time_created", &time_created));

        self.db.execute_named("
            INSERT INTO posts ( title,  url,  user,  channel,  time_created)
            VALUES            (:title, :url, :user, :channel, :time_created)",
            &params
        )?;

        Ok(())
    }

    pub fn check_prepost(&self, url: &str) -> Result<Option<PrevPost>, Error> {
        let mut st = self.db.prepare("
            SELECT user, time_created, channel
            FROM posts
            WHERE url LIKE :url
        ")?;
        let rows = st.query_named(&[(":url", &url)])?;
        let mut rows = from_rows::<PrevPost>(rows);

        Ok(rows.next())
    }

    pub fn log_error(&self, error: &UrlError) -> Result<(), Error> {
        let params = to_params_named(error).map_err(SyncFailure::new)?;
        let params = params.to_slice();

        self.db.execute_named("
            INSERT INTO errors ( url,  error,  headers,  status)
            VALUES             (:url, :error, :headers, :status)",
            &params
        )?;

        Ok(())
    }
}

#[derive(Debug, Serialize)]
pub struct NewLogEntry<'a> {
    pub title: &'a str,
    pub url: &'a str,
    pub user: &'a str,
    pub channel: &'a str,
}

#[derive(Debug, Default, Deserialize)]
pub struct PrevPost {
    pub user: String,
    pub time_created: String,
    pub channel: String
}

#[derive(Debug, Serialize)]
pub struct UrlError<'a> {
    pub error: &'a str,
    pub url: &'a str,
    pub status: &'a str,
    pub headers: &'a str,
}
