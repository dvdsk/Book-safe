use std::env::current_exe;
use std::process::Command;
use std::time::Duration;
use std::{fs, thread};

use color_eyre::eyre;
use eyre::{eyre, Result, WrapErr};

use crate::util;

pub fn ui_action(operation: &str) -> Result<()> {
    let output = Command::new("systemctl")
        .arg(operation)
        .arg("xochitl")
        .output()
        .wrap_err("Could not run systemctl")?;

    if !output.status.success() {
        let reason = String::from_utf8(output.stderr).unwrap();
        return Err(eyre!("{reason}").wrap_err("Systemctl returned an error"));
    }

    let target_activity = match operation {
        "stop" => true,
        "start" => false,
        _ => unreachable!(),
    };
    wait_for("xochitl", target_activity).wrap_err("action did not complete in time")?;

    Ok(())
}

fn is_active(service: &str) -> Result<bool> {
    let output = Command::new("systemctl")
        .arg("is-active")
        .arg(service)
        .output()
        .wrap_err("Could not run systemctl")?;

    Ok(output.status.code().unwrap() == 0)
}

fn wait_for(service: &str, state: bool) -> Result<()> {
    for _ in 0..20 {
        if state == is_active(service)? {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(50));
    }
    Err(eyre!("timed out after 1 second"))
}

// String should be written to a .service file
fn service_str(_args: &crate::Args) -> Result<String> {
    let path = current_exe().wrap_err(concat!(
        "Could not get ",
        env!("CARGO_PKG_NAME"),
        "'s binary location"
    ))?;

    let working_dir = path.parent().unwrap().to_str().unwrap();
    let bin_path = path.to_str().unwrap();
    let args: String = std::env::args().skip(1)
        .map(|mut s| {
            s.push(' ');
            s
        })
        .collect();

    Ok(format!(
        "[Unit]
Description=Makes folders in ui inaccesible for given period

[Service]
Type=oneshot
WorkingDirectory={working_dir}
ExecStart={bin_path} run {args}

[Install]
WantedBy=multi-user.target
",
    ))
}

pub fn write_service(args: &crate::Args) -> Result<()> {
    let service = service_str(args).wrap_err("Could not construct service")?;
    let path = concat!("/etc/systemd/system/", env!("CARGO_PKG_NAME"), ".service");
    fs::write(path, service).wrap_err_with(|| format!("could not write file to: {path}"))?;
    Ok(())
}

// String should be written to a .timer file
fn timer_str(args: &crate::Args) -> Result<String> {
    let start = util::try_to_time(&args.start).wrap_err("Invalid start time")?;
    let end = util::try_to_time(&args.end).wrap_err("Invalid end time")?;
    // default systemd accuracy is 1 minute for power consumption reasons
    // therefore we add one minute and some seconds to both times to ensure
    // hiding or unhiding happens
    let run_hide = format!("*-*-* {}:{}:10", start.hour(), start.minute() + 1);
    let run_unhide = format!("*-*-* {}:{}:10", end.hour(), end.minute() + 1);

    Ok(format!(
        "[Unit]
Description=Hide folders in ui at certain times

[Timer]
OnCalendar={run_hide}
OnCalendar={run_unhide}
AccuracySec=60

[Install]
WantedBy=timers.target
"
    ))
}

// TODO replace with macro for service and timer
pub fn write_timer(args: &crate::Args) -> Result<()> {
    let timer = timer_str(args).wrap_err("Could not construct timer")?;
    let path = concat!("/etc/systemd/system/", env!("CARGO_PKG_NAME"), ".timer");
    fs::write(path, timer).wrap_err_with(|| format!("could not write file to: {path}"))?;
    Ok(())
}

pub fn enable() -> Result<()> {
    let timer = concat!(env!("CARGO_PKG_NAME"), ".timer");
    let output = Command::new("systemctl")
        .arg("enable")
        .arg("--now")
        .arg(timer)
        .output()
        .wrap_err("Could not run systemctl")?;

    if !output.status.success() {
        let reason = String::from_utf8(output.stderr).unwrap();
        return Err(eyre!("{reason}").wrap_err("Systemctl returned an error"));
    }

    wait_for(timer, true).wrap_err("Timer was not activated")?;

    Ok(())
}
