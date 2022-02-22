use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use clap::{Parser, Subcommand};
use color_eyre::eyre;
use eyre::{Result, WrapErr};
use time::{OffsetDateTime, Time};

use directory::Uuid;
use util::AcceptErr;

mod directory;
mod report;
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
    Unlock,
}

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

fn move_doc(uuid: Uuid) -> Result<()> {
    let dir = Path::new(directory::DIR);

    let source = dir.join(&uuid);
    let dest = safe_dir().join(&uuid);
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
        let dest = safe_dir().join(&uuid).with_extension(ext);
        fs::rename(source, dest)
            .accept_fn(|e| e.kind() == ErrorKind::NotFound) // there isnt always content and/or pdf file
            .wrap_err_with(|| format!("Could not move file with ext: {ext:?}"))?;
    }
    Ok(())
}

fn safe_dir() -> &'static Path {
    Path::new("locked_books")
}

fn ensure_safe_dir() -> Result<()> {
    fs::create_dir(safe_dir())
        .accept_fn(|e| e.kind() == ErrorKind::AlreadyExists && safe_dir().is_dir())
        .wrap_err("Could not create books safe")
}

fn move_docs(mut to_lock: Vec<Uuid>) -> Result<()> {
    for uuid in to_lock.drain(..) {
        move_doc(uuid).wrap_err("Could not move document")?;
    }
    Ok(())
}

fn unlock() -> Result<()> {
    #[cfg(target_arch = "arm")]
    systemd::ui_action("stop").wrap_err("Could not stop gui")?;
    let dir = Path::new(directory::DIR);
    for entry in fs::read_dir(safe_dir())? {
        let entry = entry?;
        let source = entry.path();
        let dest = dir.join(source.file_name().unwrap());
        fs::rename(source, dest)?;
    }
    #[cfg(target_arch = "arm")]
    systemd::reset_failed()?;
    #[cfg(target_arch = "arm")]
    systemd::ui_action("start").wrap_err("Could not start gui")?;
    Ok(())
}

fn lock(mut forbidden: Vec<String>, unlock_at: Time) -> Result<()> {
    unlock().wrap_err("could not unlock files")?; // ensure nothing is in locked folder
    let (tree, _) = directory::map().wrap_err("Could not build document tree")?;
    let mut to_lock = Vec::new();

    let roots: Vec<_> = forbidden
        .drain(..)
        .map(|p| tree.node_for(&p))
        .collect::<Result<_>>()?;
    for node in &roots {
        let mut files = tree.descendant_files(*node)?;
        to_lock.append(&mut files);
    }
    move_docs(to_lock).wrap_err("Could not move book data")?;
    let pdf = report::build(tree, roots, unlock_at);
    report::save(pdf).wrap_err("Could not save generated report")?;

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

    ensure_safe_dir()?;
    match cli.command {
        Commands::Run(args) => run(args).wrap_err("Error while running"),
        Commands::Install(args) => install(args).wrap_err("Error while installing"),
        Commands::Remove => remove().wrap_err("Error while removing"),
        Commands::Unlock => unlock().wrap_err("Error unlocking files"),
    }
}

fn run(args: Args) -> Result<()> {
    let start = util::try_to_time(&args.start).wrap_err("Invalid start time")?;
    let end = util::try_to_time(&args.end).wrap_err("Invalid end time")?;
    let now = OffsetDateTime::now_local()
        .wrap_err("Could not get time")?
        .time();

    let forbidden = without_overlapping(args.lock);

    #[cfg(target_arch = "arm")]
    systemd::ui_action("stop").wrap_err("Could not stop gui")?;
    if util::should_lock(now, start, end) {
        println!("system time: {now}, locking folders");
        lock(forbidden, end).wrap_err("Could not lock forbidden folders")?;
    } else {
        println!("system time: {now}, unlocking everything");
        unlock().wrap_err("Could not unlock all files")?;
    }
    #[cfg(target_arch = "arm")]
    systemd::reset_failed()?;
    #[cfg(target_arch = "arm")]
    systemd::ui_action("start").wrap_err("Could not start gui")?;

    Ok(())
}

fn install(args: Args) -> Result<()> {
    systemd::write_service(&args).wrap_err("Error creating service")?;
    systemd::write_timer(&args).wrap_err("Error creating timer")?;
    systemd::enable().wrap_err("Error enabling service timer")?;
    unlock().wrap_err("Error unlocking any locked documents")?;
    run(args).wrap_err("Failed first run after install")
}

fn remove() -> Result<()> {
    systemd::disable().wrap_err("Error disabling service")?;
    systemd::remove_units().wrap_err("Error removing service files")?;
    unlock().wrap_err("Error unlocking any locked documents")
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
