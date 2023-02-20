#[cfg(target_arch = "arm")]
use color_eyre::{eyre, Help, SectionExt};
use color_eyre::{eyre::WrapErr, Result};

#[cfg(target_arch = "arm")]
use std::process::Output;
use std::process::Command;

use std::{collections::HashSet, net::IpAddr, str::FromStr};

#[cfg(target_arch = "arm")]
fn handle_any_error(
    output: Output,
    address: &IpAddr,
    text: &'static str,
) -> std::result::Result<(), Error> {
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        Err(Error::Run(
            eyre::eyre!(text)
                .with_section(move || stdout.trim().to_string().header("Stdout:"))
                .with_section(move || stderr.trim().to_string().header("Stderr:"))
                .with_section(move || format!("adress: {address}")),
        ))
    } else {
        Ok(())
    }
}

#[cfg(target_arch = "arm")]
pub fn block(address: &IpAddr) -> std::result::Result<(), Error> {
    log::debug!("blocking: {address}");
    let output = Command::new("route")
        .arg("add")
        .arg("-host")
        .arg(address.to_string())
        .arg("reject")
        .output()
        .map_err(Error::Start)?;
    handle_any_error(output, address, "Command route add returned an error")
}

#[cfg(target_arch = "arm")]
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("could not run route program")]
    Start(std::io::Error),
    #[error("route run into an error while warning: {0}")]
    Run(eyre::Report),
    #[error("Could not verify change was applied: {0}")]
    Verifying(eyre::Report),
    #[error("Operation was not applied")]
    NoEffect,
}

#[cfg(target_arch = "arm")]
pub fn unblock(address: &IpAddr) -> std::result::Result<(), Error> {
    log::debug!("unblocking: {address}");
    let output = Command::new("route")
        .arg("delete")
        .arg("-host")
        .arg(address.to_string())
        .arg("reject")
        .output()
        .map_err(Error::Start)?;
    handle_any_error(output, address, "Command route delete returned an error")?;

    let routes = table()
        .wrap_err("Error parsing routing table")
        .map_err(Error::Verifying)?;
    if routes.contains(address) {
        Err(Error::NoEffect)
    } else {
        Ok(())
    }
}

pub fn table() -> Result<HashSet<IpAddr>> {
    let output = Command::new("route")
        .arg("-n")
        .output()
        .wrap_err("Could not run route")?;

    let output = String::from_utf8_lossy(&output.stdout);
    let routes: Result<HashSet<IpAddr>, _> = output
        .lines()
        .skip(2)
        .map(|f| f.split_once(' ').unwrap().0)
        .map(IpAddr::from_str)
        .collect();
    log::debug!("parsed routes: {routes:?}");
    routes.wrap_err("Could not parse routing table entries")
}
