use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use clap::{ArgAction, Parser, Subcommand};
use color_eyre::eyre;
use eyre::{Result, WrapErr};
use itertools::Itertools;
use log::warn;
use simplelog::ConfigBuilder;
use time::{OffsetDateTime, Time};

use directory::Uuid;
use util::AcceptErr;

use crate::util::time::{set_os_timezone, should_lock, try_to_time};

mod directory;
mod report;
mod sync;
mod systemd;
mod util;

#[derive(Parser, Debug)]
pub struct Args {
    /// Path of a folder to be locked (as seen in the ui),
    /// pass multiple times to block multiple folders
    #[clap(short, long)]
    lock: Vec<String>,

    /// When to hide folders, format: 23:59
    #[clap(short, long)]
    start: String,

    /// When to release folders, format: 23:59
    #[clap(short, long)]
    end: String,

    /// Timezone, needed as remarkable resets the device's
    /// timezone to UTC on every update
    #[clap(short('z'), long)]
    timezone: String,

    /// Do not block sync when locking books, the sync will
    /// delete and re-upload books when locking and unlocking!
    #[clap(long, action = ArgAction::SetTrue)]
    allow_sync: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Lock or unlock right now depending on the time
    Run(Args),
    /// Create and enable book-safe system service, locking and unlocking
    /// at those times.
    /// This command requires additional arguments, call
    /// it with --help to see them
    Install(Args),
    /// Remove book-safe service and unlock all files. This command
    /// requires additional arguments, call it with --help to see them
    Uninstall,
    /// Unlock all files
    Unlock,
    /// List supported timezones
    ListTz { search: Option<String> },
}

#[derive(Parser, Debug)]
#[clap(
    author,
    version,
    about,
    long_about = "Hides the content of one or more folders from the remarkable ui between a given time period and adds a pdf listing what has been blocked. Cloud sync is disabled while folders are blocked. It can be ran manually with Run and Unlock or set up to trigger at given times using Install."
)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
    /// Log verbosity, used for debugging.
    /// Options: trace, debug, info, warn, error
    #[clap(short, long, default_value = "info")]
    log: simplelog::Level,
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

fn unlock_files() -> Result<()> {
    let dir = Path::new(directory::DIR);
    for entry in fs::read_dir(safe_dir())? {
        let entry = entry?;
        let source = entry.path();
        let dest = dir.join(source.file_name().unwrap());
        fs::rename(source, dest)?;
    }
    Ok(())
}

fn locked_files() -> Result<bool> {
    Ok(fs::read_dir(safe_dir())?.next().is_some())
}

fn unlock() -> Result<()> {
    if locked_files()? {
        systemd::ui_action("stop").wrap_err("Could not stop gui")?;
        unlock_files()?;
        report::remove().wrap_err("Could not remove locked files report")?;
        systemd::reset_failed()?;
        systemd::ui_action("start").wrap_err("Could not start gui")?;
    } else {
        log::info!("no files to unlock")
    }

    sync::unblock().wrap_err("Could not unblock sync")
}

fn lock(mut forbidden: Vec<String>, unlock_at: Time, allow_sync: bool) -> Result<()> {
    systemd::ui_action("stop").wrap_err("Could not stop gui")?;
    {
        unlock_files().wrap_err("could not unlock files")?; // ensure nothing is in locked folder

        let (tree, _) = directory::map().wrap_err("Could not build document tree")?;
        let mut to_lock = Vec::new();

        let (roots, missing): (Vec<_>, Vec<_>) = forbidden
            .drain(..)
            .map(|p| tree.node_for(&p))
            .partition_result();
        for node in &roots {
            let mut files = tree.descendant_files(*node)?;
            to_lock.append(&mut files);
        }
        for path in &missing {
            warn!("could not find: {path}, if it was not deleted or renamed this is a bug");
        }
        if to_lock.is_empty() {
            warn!("Found nothing to lock, is folder empty?");
            return Ok(())
        }
        let pdf = report::build(tree, roots, missing, unlock_at);
        report::save(pdf).wrap_err("Could not save locked files report")?;

        if !allow_sync {
            sync::block().wrap_err("Could not block sync")?;
        }
        move_docs(to_lock).wrap_err("Could not move book data")?;
    }
    systemd::reset_failed()?;
    systemd::ui_action("start").wrap_err("Could not start gui")
}

// TODO commands: Run, Install, Uninstall. Last one does not need current args
// Install creates a systemd unit file and loads it
// Uninstall removes a systemd unit file and unloads it
fn main() -> Result<()> {
    color_eyre::install()?;
    let cli = Cli::parse();

    use simplelog::{ColorChoice, TermLogger, TerminalMode};
    let config = ConfigBuilder::new()
        .add_filter_ignore_str("trust_dns_resolver")
        .add_filter_ignore_str("trust_dns_proto")
        .build();
    TermLogger::init(
        cli.log.to_level_filter(),
        config,
        TerminalMode::Mixed,
        ColorChoice::Auto,
    )
    .unwrap();

    ensure_safe_dir()?;
    match cli.command {
        Commands::Run(args) => run(args).wrap_err("Error while running"),
        Commands::Install(args) => install(args).wrap_err("Error while installing"),
        Commands::Uninstall => remove().wrap_err("Error while removing"),
        Commands::Unlock => unlock().wrap_err("Error unlocking files"),
        Commands::ListTz { search } => util::time::list_tz(search),
    }
}

fn run(args: Args) -> Result<()> {
    set_os_timezone(&args.timezone).wrap_err("Could not change os time zone")?;
    let start = try_to_time(&args.start).wrap_err("Invalid start time")?;
    let end = try_to_time(&args.end).wrap_err("Invalid end time")?;
    let now = OffsetDateTime::now_local()
        .wrap_err("Could not get time")?
        .time();
    log::info!("system time: {now}");

    let forbidden = util::without_overlapping(args.lock);
    util::check_folders(&forbidden).wrap_err("Could not find folders")?;

    if should_lock(now, start, end) {
        log::info!("locking folders");
        lock(forbidden, end, args.allow_sync).wrap_err("Could not lock forbidden folders")?;
    } else {
        log::info!("unlocking everything");
        unlock().wrap_err("Could not unlock all files")?;
    }

    Ok(())
}

fn install(args: Args) -> Result<()> {
    set_os_timezone(&args.timezone).wrap_err("Could not change os time zone")?;
    let forbidden = util::without_overlapping(args.lock.clone());
    util::check_folders(&forbidden).wrap_err("Could not find folders")?;
    systemd::write_service().wrap_err("Error creating service")?;
    systemd::write_timer(&args).wrap_err("Error creating timer")?;
    systemd::enable().wrap_err("Error enabling service timer")?;
    run(args).wrap_err("Failed first run after install")
}

fn remove() -> Result<()> {
    systemd::disable().wrap_err("Error disabling service")?;
    systemd::remove_units().wrap_err("Error removing service files")?;
    unlock().wrap_err("Error unlocking any locked documents")
}
