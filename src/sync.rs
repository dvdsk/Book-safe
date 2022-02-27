use color_eyre::{
    eyre::{eyre, WrapErr},
    Result, Section, SectionExt,
};
use std::{process::{Command, Output}, collections::HashSet};

fn handle_error(output: Output, address: &str, text: &'static str) -> Result<()> {
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        Err(eyre!(text)
            .with_section(move || stdout.trim().to_string().header("Stdout:"))
            .with_section(move || stderr.trim().to_string().header("Stderr:"))
            .with_section(move || format!("adress: {address}")))
    } else {
        Ok(())
    }
}

fn block_route(address: &str) -> Result<()> {
    let output = Command::new("route")
        .arg("add")
        .arg("-host")
        .arg(address)
        .arg("reject")
        .output()
        .wrap_err("Could not run route")?;
    handle_error(output, address, "Command route add returned an error")
}

fn unblock_route(address: &str) -> Result<()> {
    let output = Command::new("route")
        .arg("delete")
        .arg("-host")
        .arg(address)
        .arg("reject")
        .output()
        .wrap_err("Could not run route")?;
    handle_error(output, address, "Command route delete returned an error")
}

fn parse_routes() -> Result<HashSet<String>> {
    let output = Command::new("route")
        .arg("-n")
        .output()
        .wrap_err("Could not run route")?;

    let output = String::from_utf8_lossy(&output.stdout);
    let routes: HashSet<String> = output
        .lines()
        .skip(2)
        .map(|f| f.split_once(" ").unwrap().0.to_owned())
        .collect();
    Ok(routes)
}
// TODO auto generate from routes remarkable has open
const ROUTES: [&'static str; 3] = [
    "206.137.117.34",
    "117.147.117.34",
    "172.217.168.212",
];

pub fn block() -> Result<()> {
    let existing = parse_routes().wrap_err("Error parsing routing table")?;
    for addr in ROUTES {
        if !existing.contains(addr) {
            block_route(addr)?;
        }
    }
    Ok(())
}

pub fn unblock() -> Result<()> {
    let existing = parse_routes().wrap_err("Error parsing routing table")?;
    for addr in ROUTES {
        if existing.contains(addr) {
            unblock_route(addr)?;
        }
    }
    Ok(())
}
