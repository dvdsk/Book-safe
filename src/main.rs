use std::collections::HashMap;

use clap::Parser;
use color_eyre::eyre;
use eyre::{eyre, Result, WrapErr};
use time::{OffsetDateTime, Time};

mod directory;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// names of the folders which content should be locked, comma separated names
    #[clap(short, long)]
    forbidden: Vec<String>,

    /// when to hide folders, format 23:59
    #[clap(short, long)]
    start: String,

    /// when to release folders, format 23:59
    #[clap(short, long)]
    end: String,
}

fn try_to_time(s: &str) -> Result<time::Time> {
    let (h, m) = s
        .split_once(":")
        .ok_or_else(|| eyre!("hours and minutes must be separated by :"))?;
    let h = h.parse().wrap_err("could not parse hour")?;
    let m = m.parse().wrap_err("could not parse minute")?;
    time::Time::from_hms(h, m, 0).wrap_err("hour or minute not possible")
}

fn lock(mut forbidden: Vec<String>) -> Result<()> {
    let (tree, to_fsname) = directory::map();
    let to_name: HashMap<_, _> = to_fsname
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .map(|(k, v)| (v, k))
        .collect();
    let mut to_lock = Vec::new();

    loop {
        let path = match forbidden.pop() {
            None => break,
            Some(d) => d,
        };

        let (mut files, _folder_paths) = tree.children(path)?;

        dbg!("TODO strip containing paths");
        // forbidden.retain(|f| !folders.contains(f));
        to_lock.append(&mut files);
    }
    let to_lock: Vec<_> = to_lock.iter().map(|f| to_name.get(f)).collect();
    // dbg!(to_lock);

    Ok(())
}

fn unlock(forbidden: Vec<String>) {
    todo!();
}

fn main() -> Result<()> {
    color_eyre::install()?;

    let args = Args::parse();
    let start = try_to_time(&args.start).wrap_err("invalid start time")?;
    let end = try_to_time(&args.end).wrap_err("invalid end time")?;
    let now = OffsetDateTime::now_local()
        .wrap_err("could not get time")?
        .time();

    if should_lock(now, start, end) {
        lock(args.forbidden).wrap_err("could not lock forbidden folders")?;
    } else {
        unlock(args.forbidden);
    }

    Ok(())
}

fn should_lock(now: Time, start: Time, end: Time) -> bool {
    if start <= end {
        now >= start && now <= end
    } else {
        now >= start || now <= end
    }
}

#[test]
fn time_compare() {
    let start = Time::from_hms(23, 10, 0).unwrap();
    let end = Time::from_hms(8, 5, 0).unwrap();

    let now = Time::from_hms(8, 10, 0).unwrap();
    assert!(!should_lock(now, start, end));

    let now = Time::from_hms(8, 4, 0).unwrap();
    assert!(should_lock(now, start, end));

    let now = Time::from_hms(23, 11, 0).unwrap();
    assert!(should_lock(now, start, end));

    let now = Time::from_hms(23, 09, 0).unwrap();
    assert!(!should_lock(now, start, end));
}
