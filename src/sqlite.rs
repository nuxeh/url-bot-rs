extern crate rusqlite;
extern crate time;

use self::rusqlite::Connection;
use std::process;

#[derive(Debug)]
pub struct LogEntry<'a> {
    pub id: i32,
    pub title: String,
    pub url: String,
    pub prefix: &'a String,
    pub channel: String,
    pub time_created: String,
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

pub fn create_db(path: &str) -> Option<Connection>
{
    let db = Connection::open(path).unwrap();
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
