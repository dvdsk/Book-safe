use color_eyre::{
    eyre::{eyre, WrapErr},
    Result, Section, SectionExt,
};
use std::process::{Command, Output};

fn handle_error(output: Output) -> Result<()> {
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        Err(eyre!("route add returned an error")
            .with_section(move || stdout.trim().to_string().header("Stdout:"))
            .with_section(move || stderr.trim().to_string().header("Stderr:")))
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
    handle_error(output)
}

fn unblock_route(address: &str) -> Result<()> {
    let output = Command::new("route")
        .arg("delete")
        .arg("-host")
        .arg(address)
        .arg("reject")
        .output()
        .wrap_err("Could not run route")?;
    handle_error(output)
}

const ROUTES: [&'static str; 6] = [
    "206.137.117.34",
    "117.147.117.34",
    "206.137.117.34",
    "206.137.117.34.bc.googleusercontent.com:https",
    "206.137.117.34.bc.googleusercontent.com",
    "ams16s32-in-f20.1e100.net",
];

pub fn block() -> Result<()> {
    for addr in ROUTES {
        block_route(addr)?;
    }
    Ok(())
}

pub fn unblock() -> Result<()> {
    for addr in ROUTES {
        unblock_route(addr)?;
    }
    Ok(())
}
