use rusqlite::Connection;
use failure::Error;
use time;

#[derive(Debug)]
pub struct LogEntry<'a> {
    pub id: i32,
    pub title: &'a str,
    pub url: &'a str,
    pub prefix: &'a str,
    pub channel: &'a str,
    pub time_created: &'a str,
}

pub fn add_log(db: &Connection, e: &LogEntry) -> Result<(), Error> {
    let u: Vec<_> = e.prefix
        .split("!")
        .collect();

    db.execute("INSERT INTO posts (title, url, user, channel, time_created)
        VALUES (?1, ?2, ?3, ?4, ?5)",
        &[
            &e.title,
            &e.url,
            &String::from(u[0]),
            &e.channel,
            &time::now().to_local().ctime().to_string()
        ]
    )?;

    Ok(())
}

#[derive(Debug, Default)]
pub struct PrevPost {
    pub user: String,
    pub time_created: String,
    pub channel: String
}

pub fn check_prepost(db: &Connection, e: &LogEntry) -> Result<Option<PrevPost>, Error> {
    let query = format!("SELECT user, time_created, channel
                         FROM posts
                         WHERE url LIKE \"{}\"",
            e.url.clone()
            .replace("\"", "\"\""));

    let mut st = db.prepare(&query)?;

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

pub fn create_db(path: Option<&str>) -> Result<Connection, Error> {
    let db = match path {
        Some(path) => Connection::open(path)?,
        None => Connection::open_in_memory()?,
    };

    db.execute("CREATE TABLE IF NOT EXISTS posts (
        id              INTEGER PRIMARY KEY,
        title           TEXT NOT NULL,
        url             TEXT NOT NULL,
        user            TEXT NOT NULL,
        channel         TEXT NOT NULL,
        time_created    TEXT NOT NULL
        )", &[])?;

    Ok(db)
}
