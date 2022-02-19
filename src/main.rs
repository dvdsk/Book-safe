use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use clap::{Parser, Subcommand};
use color_eyre::eyre;
use eyre::{Result, WrapErr};
use time::OffsetDateTime;

use directory::Uuid;
use util::AcceptErr;

mod directory;
mod systemd;
mod util;

#[derive(Parser, Debug)]
pub struct Args {
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

#[derive(Subcommand, Debug)]
enum Commands {
    Run(Args),
    Install(Args),
    Remove,
}

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
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
    let (tree, _) = directory::map().wrap_err("Could not build document tree")?;
    let mut to_lock = Vec::new();

    loop {
        let path = match forbidden.pop() {
            None => break,
            Some(d) => d,
        };

        let mut files = tree.children(path)?;
        to_lock.append(&mut files);
    }
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

// TODO commands: Run, Install, Uninstall. Last one does not need current args
// Install creates a systemd unit file and loads it
// Uninstall removes a systemd unit file and unloads it
fn main() -> Result<()> {
    color_eyre::install()?;
    let cli = Cli::parse();
    match cli.command {
        Commands::Run(args) => run(args).wrap_err("Error while running")?,
        Commands::Install(args) => {
            install(&args).wrap_err("Error while installing")?;
            run(args).wrap_err("Error running after install")?;
        }
        Commands::Remove => todo!(),
    }
    Ok(())
}

fn install(args: &Args) -> Result<()> {
    systemd::write_service(args).wrap_err("Error creating service")?;
    systemd::write_timer(args).wrap_err("Error creating timer")?;
    systemd::enable().wrap_err("Error enabling service timer")?;
    Ok(())
}

fn run(args: Args) -> Result<()> {
    let start = util::try_to_time(&args.start).wrap_err("Invalid start time")?;
    let end = util::try_to_time(&args.end).wrap_err("Invalid end time")?;
    let now = OffsetDateTime::now_local()
        .wrap_err("Could not get time")?
        .time();

    let forbidden = without_overlapping(args.lock);

    systemd::ui_action("stop").wrap_err("Could not stop gui")?;
    if util::should_lock(now, start, end) {
        println!("locking folders");
        lock(forbidden).wrap_err("Could not lock forbidden folders")?;
    } else {
        println!("unlocking everything");
        unlock().wrap_err("Could not unlock all files")?;
    }
    systemd::ui_action("start").wrap_err("Could not start gui")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::Time;

    #[test]
    fn time_compare() {
        let start = Time::from_hms(23, 10, 0).unwrap();
        let end = Time::from_hms(8, 5, 0).unwrap();

        let now = Time::from_hms(8, 10, 0).unwrap();
        assert!(!util::should_lock(now, start, end));

        let now = Time::from_hms(8, 4, 0).unwrap();
        assert!(util::should_lock(now, start, end));

        let now = Time::from_hms(23, 11, 0).unwrap();
        assert!(util::should_lock(now, start, end));

        let now = Time::from_hms(23, 09, 0).unwrap();
        assert!(!util::should_lock(now, start, end));
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
}
