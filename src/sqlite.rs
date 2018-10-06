use rusqlite::Connection;
use failure::Error;
use time;
use std::path::Path;

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

        Ok(Self { db })
    }

    pub fn add_log(&self, entry: &LogEntry) -> Result<(), Error> {
        let u: Vec<_> = entry.prefix
            .split("!")
            .collect();

        self.db.execute("INSERT INTO posts (title, url, user, channel, time_created)
            VALUES (?1, ?2, ?3, ?4, ?5)",
            &[
                &entry.title,
                &entry.url,
                &String::from(u[0]),
                &entry.channel,
                &time::now().to_local().ctime().to_string()
            ]
        )?;

        Ok(())
    }

    pub fn check_prepost(&self, e: &LogEntry) -> Result<Option<PrevPost>, Error> {
        let query = format!("SELECT user, time_created, channel
                            FROM posts
                            WHERE url LIKE \"{}\"",
                e.url.clone()
                .replace("\"", "\"\""));

        let mut st = self.db.prepare(&query)?;

        let mut res = st.query_map(&[], |r| {
            PrevPost {
                user: r.get(0),
                time_created: r.get(1),
                channel: r.get(2)
            }
        })?;

        Ok(match res.nth(0) {
            Some(post) => Some(post?),
            None    => None
        })
    }
}

#[derive(Debug)]
pub struct LogEntry<'a> {
    pub id: i32,
    pub title: &'a str,
    pub url: &'a str,
    pub prefix: &'a str,
    pub channel: &'a str,
    pub time_created: &'a str,
}

#[derive(Debug, Default)]
pub struct PrevPost {
    pub user: String,
    pub time_created: String,
    pub channel: String
}
