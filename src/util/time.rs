use color_eyre::{
    eyre::{eyre, WrapErr},
    Help, Result,
};
use itertools::Itertools;
use rust_fuzzy_search::fuzzy_search_best_n;
use std::{io::BufRead, process::Command};
use time::Time;

pub trait ParseHourMinute {
    fn try_parse(s: &str) -> Result<time::Time>;
}

impl ParseHourMinute for time::Time {
    fn try_parse(s: &str) -> Result<time::Time> {
        let (h, m) = s
            .split_once(':')
            .ok_or_else(|| eyre!("Hours and minutes must be separated by :"))?;
        let h = h.parse().wrap_err("Could not parse hour")?;
        let m = m.parse().wrap_err("Could not parse minute")?;
        time::Time::from_hms(h, m, 0).wrap_err("Hour or minute not possible")
    }
}

pub fn should_lock(now: Time, start: Time, end: Time) -> bool {
    if start <= end {
        now >= start && now <= end
    } else {
        now >= start || now <= end
    }
}

pub fn set_os_timezone(timezone: &str) -> Result<()> {
    let output = Command::new("timedatectl")
        .arg("set-timezone")
        .arg(timezone)
        .output()
        .wrap_err("Could not run timedatectl")?;

    if output.status.success() {
        Ok(())
    } else {
        let reason = String::from_utf8(output.stderr).unwrap();
        let timezones = get_timezones().wrap_err("Could not get time zones for suggestion")?;
        let report = eyre!("{reason}");
        Err(match list_fuzzy(&timezones, timezone, 1).get(0) {
            Some(sugg) => report.suggestion(format!("did you mean: \"{sugg}\"",)),
            None => report,
        })
    }
}

fn get_timezones() -> Result<Vec<String>> {
    let output = Command::new("timedatectl")
        .arg("list-timezones")
        .output()
        .wrap_err("Could not run timedatectl")?;

    if output.status.success() {
        Ok(output.stdout.lines().map(Result::unwrap).collect_vec())
    } else {
        let reason = String::from_utf8(output.stderr).unwrap();
        Err(eyre!("{reason}").wrap_err("datetimectl returned an error"))
    }
}

fn list_fuzzy<'a>(timezones: &'a [String], term: &'a str, n: usize) -> Vec<String> {
    let options = timezones.iter().map(String::as_str).collect_vec();
    let mut results = fuzzy_search_best_n(term, &options, n);
    results.sort_unstable_by(|(_, score_a), (_, score_b)| {
        score_a
            .partial_cmp(score_b)
            .expect("nan should not occure in search score")
    });

    results
        .into_iter()
        .filter(|(_, score)| *score > 0.4)
        .map(|(name, _)| name)
        .map(str::to_owned)
        .collect_vec()
}

pub(crate) fn list_tz(search: Option<String>) -> Result<(), color_eyre::Report> {
    let mut timezones = get_timezones().wrap_err("Could not get timezones")?;
    if let Some(term) = search {
        timezones = list_fuzzy(&timezones, &term, 10);
    }
    for name in timezones {
        println!("{name}");
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

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
}
