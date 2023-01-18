use std::env::current_exe;
use std::process::Command;
use std::time::Duration;
use std::{fs, thread};

use color_eyre::eyre;
use eyre::{eyre, Result, WrapErr};

use crate::util::time::ParseHourMinute;
use time::Time;

#[cfg(not(target_arch = "arm"))]
#[allow(clippy::unnecessary_wraps)]
pub fn reset_failed() -> Result<()> {
    Ok(())
}
#[cfg(target_arch = "arm")]
#[allow(clippy::unnecessary_wraps)]
pub fn reset_failed() -> Result<()> {
    systemctl(&["reset-failed"], "xochitl")?;
    Ok(())
}

#[cfg(not(target_arch = "arm"))]
#[allow(clippy::unnecessary_wraps)]
pub fn ui_action(_operation: &'static str) -> Result<()> {
    Ok(())
}

#[cfg(target_arch = "arm")]
pub fn ui_action(operation: &'static str) -> Result<()> {
    systemctl(&[operation], "xochitl")?;

    let target_activity = match operation {
        "start" => {
            log::info!("starting ui");
            true
        }
        "stop" => {
            log::info!("stopping ui");
            false
        }
        _ => unreachable!(),
    };
    wait_for("xochitl", target_activity).wrap_err("operation did not complete in time")?;
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

fn wait_for(service: &str, wanted_active: bool) -> Result<()> {
    for _ in 0..20 {
        match (wanted_active, is_active(service)?) {
            (true, true) | (false, false) => return Ok(()),
            (true, false) | (false, true) => (),
        };
        thread::sleep(Duration::from_millis(50));
    }

    Err(eyre!(
        "Time out waiting for {}",
        match wanted_active {
            true => "activation",
            false => "deactivation",
        }
    ))
}

// String should be written to a .service file
fn service_str() -> Result<String> {
    let path = current_exe().wrap_err(concat!(
        "Could not get ",
        env!("CARGO_PKG_NAME"),
        "'s binary location"
    ))?;

    let working_dir = path.parent().unwrap().to_str().unwrap();
    let bin_path = path.to_str().unwrap();
    let args: String = std::env::args()
        .skip(1) // skip binary name
        .map(|s| {
            if s.contains(' ') {
                format!("\"{s}\" ")
            } else {
                format!("{s} ")
            }
        })
        .collect();
    let args = args.replace("install", "run");

    Ok(format!(
        "[Unit]
Description=Makes folders in ui inaccesible for given period

[Service]
Type=oneshot
WorkingDirectory={working_dir}
ExecStart={bin_path} {args}

[Install]
WantedBy=multi-user.target
",
    ))
}

macro_rules! unit_path {
    ($ext:literal) => {
        concat!("/etc/systemd/system/", env!("CARGO_PKG_NAME"), ".", $ext)
    };
}

pub fn write_service() -> Result<()> {
    let service = service_str().wrap_err("Could not construct service")?;
    let path = unit_path!("service");
    fs::write(path, service).wrap_err_with(|| format!("could not write file to: {path}"))?;
    Ok(())
}

// String should be written to a .timer file
fn timer_str(args: &crate::Args) -> Result<String> {
    let start = Time::try_parse(&args.start).wrap_err("Invalid start time")?;
    let end = Time::try_parse(&args.end).wrap_err("Invalid end time")?;
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

pub fn write_timer(args: &crate::Args) -> Result<()> {
    let timer = timer_str(args).wrap_err("Could not construct timer")?;
    let path = unit_path!("timer");
    fs::write(path, timer).wrap_err_with(|| format!("could not write file to: {path}"))
}

pub fn remove_units() -> Result<()> {
    fs::remove_file(unit_path!("timer")).wrap_err("Error removing timer")?;
    fs::remove_file(unit_path!("service")).wrap_err("Error removing service")
}

fn systemctl(args: &[&'static str], service: &str) -> Result<()> {
    let output = Command::new("systemctl")
        .args(args)
        .arg(service)
        .output()
        .wrap_err("Could not run systemctl")?;

    if output.status.success() {
        Ok(())
    } else {
        let reason = String::from_utf8(output.stderr).unwrap();
        Err(eyre!("{reason}").wrap_err("Systemctl returned an error"))
    }
}

fn timer() -> &'static str {
    concat!(env!("CARGO_PKG_NAME"), ".timer")
}

pub fn enable() -> Result<()> {
    systemctl(&["enable", "--now"], timer())?;
    wait_for(timer(), true).wrap_err("Timer was not activated")?;
    Ok(())
}

pub fn disable() -> Result<()> {
    systemctl(&["disable", "--now"], timer())?;
    wait_for(timer(), false).wrap_err("Timer was not deactivated")?;
    Ok(())
}
