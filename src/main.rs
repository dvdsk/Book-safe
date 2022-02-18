use std::collections::HashMap;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::process::Command;

use clap::Parser;
use color_eyre::eyre;
use eyre::{eyre, Result, WrapErr};
use time::{OffsetDateTime, Time};

use self::directory::Uuid;

mod directory;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// name of a folder to be locked, argument can be passed
    /// multiple times with diffrent folders
    #[clap(short, long)]
    lock: Vec<String>,

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
        .ok_or_else(|| eyre!("Hours and minutes must be separated by :"))?;
    let h = h.parse().wrap_err("Could not parse hour")?;
    let m = m.parse().wrap_err("Could not parse minute")?;
    time::Time::from_hms(h, m, 0).wrap_err("Hour or minute not possible")
}

fn move_doc(uuid: Uuid) -> Result<()> {
    let dir = Path::new(directory::DIR);
    let safe = Path::new("locked_books");

    let source = dir.join(&uuid);
    let dest = safe.join(&uuid);
    fs::rename(source, dest)
        .accept_fn(|e| e.kind() == ErrorKind::NotFound) // there isnt always content and/or pdf file
        .wrap_err_with(|| format!("Could not move directory for document: {uuid}"))?;

    for ext in [
        "bookm",
        "content",
        "epub",
        "epubindex",
        "metadata",
        "pagedata",
        "pdf",
    ] {
        let source = dir.join(&uuid).with_extension(ext);
        let dest = safe.join(&uuid).with_extension(ext);
        fs::rename(source, dest)
            .accept_fn(|e| e.kind() == ErrorKind::NotFound) // there isnt always content and/or pdf file
            .wrap_err_with(|| format!("Could not move file with ext: {ext:?}"))?;
    }
    Ok(())
}

pub trait AcceptErr {
    type Error;
    fn accept_fn<P: FnMut(&Self::Error) -> bool>(self, predicate: P) -> Self;
}

impl<E> AcceptErr for Result<(), E> {
    type Error = E;
    fn accept_fn<P: FnMut(&Self::Error) -> bool>(self, mut predicate: P) -> Self {
        match self {
            Ok(_) => Ok(()),
            Err(e) if predicate(&e) => Ok(()),
            Err(e) => Err(e),
        }
    }
}

fn move_docs(mut to_lock: Vec<Uuid>) -> Result<()> {
    let safe = Path::new("locked_books");
    fs::create_dir(safe)
        .accept_fn(|e| e.kind() == ErrorKind::AlreadyExists && safe.is_dir())
        .wrap_err("Could not create books safe")?;

    for uuid in to_lock.drain(..) {
        move_doc(uuid).wrap_err("Could not move document")?;
    }

    Ok(())
}

fn unlock() -> Result<()> {
    let safe = Path::new("locked_books");
    let dir = Path::new(directory::DIR);
    for entry in fs::read_dir(safe)? {
        let entry = entry?;
        let source = entry.path();
        let dest = dir.join(source.file_name().unwrap());
        fs::rename(source, dest)?;
    }
    Ok(())
}

fn lock(mut forbidden: Vec<String>) -> Result<()> {
    let (tree, to_fsname) = directory::map().wrap_err("Could not build document tree")?;
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

        let mut files = tree.children(path)?;
        to_lock.append(&mut files);
    }
    // let names: Vec<_> = to_lock.iter().map(|f| to_name.get(f)).collect();
    // dbg!(names);
    move_docs(to_lock).wrap_err("Could not move book data")?;

    Ok(())
}

fn without_overlapping(mut list: Vec<String>) -> Vec<String> {
    let mut result = Vec::new();
    list.sort_unstable_by_key(String::len);

    for path in list.drain(..) {
        if !result.iter().any(|prefix| path.starts_with(prefix)) {
            result.push(path);
        }
    }
    result
}

fn systemctl_gui(command: &str) -> Result<()> {
    let output = Command::new("systemctl")
        .arg("stop")
        .arg(command)
        .output()
        .wrap_err("Could not run systemctl")?;

    if !output.status.success() {
        let reason = String::from_utf8(output.stderr).unwrap();
        return Err(eyre!("{reason}").wrap_err("Systemctl returned an error"))
    }

    Ok(())
}

fn main() -> Result<()> {
    color_eyre::install()?;

    let args = Args::parse();
    let start = try_to_time(&args.start).wrap_err("Invalid start time")?;
    let end = try_to_time(&args.end).wrap_err("Invalid end time")?;
    let now = OffsetDateTime::now_local()
        .wrap_err("Could not get time")?
        .time();

    let forbidden = without_overlapping(args.lock);

    systemctl_gui("stop").wrap_err("Could not stop gui")?;
    if should_lock(now, start, end) {
        lock(forbidden).wrap_err("Could not lock forbidden folders")?;
    } else {
        unlock().wrap_err("Could not unlock all files")?;
    }
    systemctl_gui("start").wrap_err("Could not start gui")?;

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

#[cfg(test)]
fn vec(list: &[&str]) -> Vec<String> {
    list.into_iter().map(|s| s.to_string()).collect()
}

#[test]
fn overlapping_paths() {
    let list = vec(&["a/aa/aaa", "b/bb", "a/aa/aab", "b/ba"]);
    let res = vec(&["b/bb", "b/ba", "a/aa/aaa", "a/aa/aab"]);
    assert_eq!(res, without_overlapping(list));

    let list = vec(&["a/aa", "b/bb", "a/aa/aab", "b/ba"]);
    let res = vec(&["a/aa", "b/bb", "b/ba"]);
    assert_eq!(res, without_overlapping(list));

    let list = vec(&["Books", "Books/Stories"]);
    let res = vec(&["Books"]);
    assert_eq!(res, without_overlapping(list));
}
