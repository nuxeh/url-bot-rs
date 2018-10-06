extern crate rusqlite;
extern crate time;

use self::rusqlite::Connection;
use std::process;

#[derive(Debug)]
pub struct LogEntry<'a> {
    pub id: i32,
    pub title: &'a str,
    pub url: &'a str,
    pub prefix: &'a str,
    pub channel: &'a str,
    pub time_created: &'a str,
}

pub fn add_log(db: &Connection, e: &LogEntry) {
    let u: Vec<_> = e.prefix
        .split("!")
        .collect();
    match db.execute("INSERT INTO posts (title, url, user, channel, time_created)
        VALUES (?1, ?2, ?3, ?4, ?5)",
        &[&e.title,
          &e.url,
          &String::from(u[0]),
          &e.channel,
          &time::now().to_local().ctime().to_string()])
    {
        Err(e) => {eprintln!("SQL error: {}", e); process::exit(1)},
        _      => (),
    }
}

#[derive(Debug, Default)]
pub struct PrevPost {
    pub user: String,
    pub time_created: String,
    pub channel: String
}

pub fn check_prepost(db: &Connection, e: &LogEntry) -> Option<PrevPost>
{
    let query = format!("SELECT user, time_created, channel
                         FROM posts
                         WHERE url LIKE \"{}\"",
            e.url.clone()
            .replace("\"", "\"\""));

    let mut st = db.prepare(&query).unwrap();

    let mut res = st.query_map(&[], |r| {
        PrevPost {
            user: r.get(0),
            time_created: r.get(1),
            channel: r.get(2)
        }
        }).unwrap();

    match res.nth(0) {
        Some(r) => Some(r.unwrap()),
        None    => None
    }
}

pub fn create_db(path: Option<&str>) -> Option<Connection> {
    let db = match path {
        Some(path) => Connection::open(path).unwrap(),
        None => Connection::open_in_memory().unwrap(),
    };

    db.execute("CREATE TABLE IF NOT EXISTS posts (
        id              INTEGER PRIMARY KEY,
        title           TEXT NOT NULL,
        url             TEXT NOT NULL,
        user            TEXT NOT NULL,
        channel         TEXT NOT NULL,
        time_created    TEXT NOT NULL
        )", &[]).unwrap();

    Some(db)
}
