use color_eyre::{eyre, Help};
use eyre::{eyre, Result, WrapErr};
use itertools::Itertools;
use time::Time;
use rust_fuzzy_search::fuzzy_search_best_n;

use crate::directory;

pub trait AcceptErr {
    type Error;
    fn accept_fn<P: FnMut(&Self::Error) -> bool>(self, predicate: P) -> Self;
}

impl<E> AcceptErr for Result<(), E> {
    type Error = E;
    fn accept_fn<P: FnMut(&Self::Error) -> bool>(self, mut predicate: P) -> Self {
        match self {
            Ok(_) => Ok(()),
            Err(e) if predicate(&e) => Ok(()),
            Err(e) => Err(e),
        }
    }
}

pub fn try_to_time(s: &str) -> Result<time::Time> {
    let (h, m) = s
        .split_once(":")
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

pub fn without_overlapping(mut list: Vec<String>) -> Vec<String> {
    let mut result = Vec::new();
    list.sort_unstable_by_key(String::len);

    for path in list.drain(..) {
        if !result.iter().any(|prefix| path.starts_with(prefix)) {
            result.push(path);
        }
    }
    result
}

fn path_suggestion<'a>(path: String, paths: &'a [String]) -> Option<String> {
    let paths: Vec<_> = paths.iter().map(|s| s.as_str()).collect();
    let results = fuzzy_search_best_n(&path, &paths, 1);
    let (candidate, score) = results.get(0)?;
    if *score > 0.8 {
        Some(candidate.to_string())
    } else {
        None
    }
}

pub fn check_folders(forbidden: &[String]) -> Result<()> {
    let (tree, index) = directory::map().wrap_err("Could not build document tree")?;
    let names: Vec<_> = index.into_keys().collect();

    let missing: Vec<_> = forbidden
        .iter()
        .map(|p| tree.node_for(p))
        .filter_map(Result::err)
        .collect();

    if missing.is_empty() {
        return Ok(());
    }

    let mut report = eyre!("Not every path that should be locked exist");
    for path in missing {
        report = report.section(format!("Could not find: \"{path}\""));
        if let Some(sug) = path_suggestion(path, &names) {
            report = report.suggestion(format!("did you mean: \"{sug}\""));
        }
    }
    Err(report)
}

#[cfg(test)]
mod test {
    use super::*;
    use float_eq::assert_float_eq;

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

    #[test]
    fn suggestions() {
        let paths = vec![
            "Referece Textbooks",
            "Cources",
            "Personal",
            "Cheat sheets",
            "Hobby",
        ]
        .into_iter()
        .map(ToOwned::to_owned)
        .collect_vec();
        let res = path_suggestion("Reference Textbooks".into(), &paths[..]);
        assert_eq!(res, Some("Referece Textbooks".to_owned()));
    }

    #[test]
    fn fuzzy_match() {
        use rust_fuzzy_search::fuzzy_compare;
        let score = fuzzy_compare("Reference textbooks",  "Referece textbooks");
        assert_float_eq!(score, 0.85, abs <= 0.001);

        let score = fuzzy_compare("Referece textbooks",  "Referece textbooks");
        assert_float_eq!(score, 1.0, abs <= 0.001);
    }

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
