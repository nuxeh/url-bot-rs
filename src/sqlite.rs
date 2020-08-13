use rusqlite::{params, Connection};
use failure::{Error, SyncFailure};
use std::path::Path;
use serde_rusqlite::{from_rows, to_params_named};
use chrono::Utc;
use serde_derive::{Serialize, Deserialize};

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
            params![],
        )?;
        db.execute("CREATE TABLE IF NOT EXISTS errors (
            id              INTEGER PRIMARY KEY,
            url             TEXT NOT NULL,
            error_info      TEXT NOT NULL
            )",
            params![],
        )?;

        Ok(Self { db })
    }

    pub fn add_log(&self, entry: &NewLogEntry) -> Result<(), Error> {
        let time_created = Utc::now().format("%a %b %-d %H:%M:%S %-Y").to_string();
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
