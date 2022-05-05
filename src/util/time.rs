use std::{process::Command, io::BufRead};
use color_eyre::{
    eyre::{eyre, WrapErr},
    Help, Result,
};
use itertools::Itertools;
use rust_fuzzy_search::fuzzy_search_best_n;
use time::Time;

pub fn try_to_time(s: &str) -> Result<time::Time> {
    let (h, m) = s
        .split_once(':')
        .ok_or_else(|| eyre!("Hours and minutes must be separated by :"))?;
    let h = h.parse().wrap_err("Could not parse hour")?;
    let m = m.parse().wrap_err("Could not parse minute")?;
    time::Time::from_hms(h, m, 0).wrap_err("Hour or minute not possible")
}

pub fn should_lock(now: Time, start: Time, end: Time) -> bool {
    if start <= end {
        now >= start && now <= end
    } else {
        now >= start || now <= end
    }
}

pub fn set_os_timezone(timezone: &str) -> Result<()> {
    let output = Command::new("datetimectl")
        .arg("set-timezone")
        .arg(timezone)
        .output()
        .wrap_err("Could not run datetimectl")?;

    if !output.status.success() {
        let reason = String::from_utf8(output.stderr).unwrap();
        let timezones = get_timezones().wrap_err("Could not get timezones for suggestion")?;
        let report = eyre!("{reason}");
        Err(match list_fuzzy(&timezones, timezone, 1).get(0) {
            Some(sugg) => report.suggestion(format!("did you mean: \"{sugg}\"",)),
            None => report,
        })
    } else {
        Ok(())
    }
}

fn get_timezones() -> Result<Vec<String>> {
    let output = Command::new("datetimectl")
        .arg("list-timezones")
        .output()
        .wrap_err("Could not run datetimectl")?;

    if !output.status.success() {
        let reason = String::from_utf8(output.stderr).unwrap();
        Err(eyre!("{reason}").wrap_err("Systemctl returned an error"))
    } else {
        Ok(output.stdout.lines().map(Result::unwrap).collect_vec())
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
        println!("{}", name);
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
