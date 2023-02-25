use color_eyre::{eyre::WrapErr, Result};
use std::io::{BufReader, BufWriter};
use std::time::{Duration, SystemTime};
use std::{fs, net::IpAddr};

use serde::{Deserialize, Serialize};

const EXPIRATION: Duration = Duration::from_secs(60 * 60 * 24 * 7 * 4 * 2);

#[derive(Debug, Serialize, Deserialize)]
pub struct Entry {
    ip: IpAddr,
    last_updated: SystemTime,
}

#[derive(Debug)]
/// route cache that might contain outdated entries
/// that should be removed
pub struct Cached(Vec<Entry>);

impl Cached {
    pub fn load() -> Result<Self> {
        let f = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open("routes.json")?;

        if f.metadata()?.len() == 0 {
            return Ok(Cached(Vec::new()));
        }

        let r = BufReader::new(f);
        let entries = serde_json::from_reader(r).wrap_err("could not parse adress in file")?;
        Ok(Cached(entries))
    }

    #[must_use]
    fn n_recent(&self) -> usize {
        self.0
            .iter()
            .filter(|e| e.last_updated.elapsed().unwrap_or(Duration::ZERO) < EXPIRATION)
            .count()
    }

    #[must_use]
    pub fn blocked_ips(self) -> Vec<IpAddr> {
        self.0.into_iter().map(|e| e.ip).collect()
    }

    #[must_use]
    pub fn update(mut self, new: Vec<IpAddr>) -> Option<Routes> {
        self.0.extend(new.into_iter().map(|ip| Entry {
            ip,
            last_updated: SystemTime::now(),
        }));
        dedup_keep_newest(&mut self.0);

        if self.0.is_empty() {
            return None;
        }

        if self.n_recent() < 2 {
            return Some(Routes(self.0));
        }

        /* TODO: replace with drain_filter once stabelized <16-02-23, dvdsk> */
        let mut i = 0;
        while i < self.0.len() {
            if let Ok(age) = self.0[i].last_updated.elapsed() {
                if age > EXPIRATION {
                    self.0.remove(i);
                    continue;
                }
            }
            i += 1;
        }

        Some(Routes(self.0))
    }
}

pub struct Routes(Vec<Entry>);

impl Routes {
    pub fn cache(&self) -> Result<()> {
        let f = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .open("routes.json")?;
        let w = BufWriter::new(f);
        serde_json::to_writer_pretty(w, &self.0)?;
        Ok(())
    }

    #[must_use]
    pub fn into_ips(self) -> Vec<IpAddr> {
        self.0.into_iter().map(|e| e.ip).collect()
    }
}

fn dedup_keep_newest(list: &mut Vec<Entry>) {
    list.sort_unstable_by_key(|e| e.last_updated);
    list.reverse();
    list.sort_by_key(|e| e.ip);
    list.dedup_by_key(|e| e.ip);
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;
    use std::time::Duration;

    use itertools::Itertools;

    use super::*;

    fn old_entry(ip: u8, age: u64) -> Entry {
        Entry {
            ip: IpAddr::V4(Ipv4Addr::new(0, 0, 0, ip)),
            last_updated: SystemTime::UNIX_EPOCH + Duration::from_secs(age),
        }
    }

    fn recent_entry(ip: u8) -> Entry {
        Entry {
            ip: IpAddr::V4(Ipv4Addr::new(0, 0, 0, ip)),
            last_updated: SystemTime::now(),
        }
    }

    mod duplicates {
        use super::*;

        #[test]
        fn oldest_removed() {
            let mut list: Vec<_> = (0..10).into_iter().map(|i| old_entry(i, 0)).collect();
            let new = (0..10).into_iter().map(|i| old_entry(i, i as u64 + 100));
            list.extend(new);

            dedup_keep_newest(&mut list);

            let duplicates = list.iter().duplicates_by(|e| e.ip);
            dbg!(&duplicates);

            assert_eq!(
                0,
                duplicates.count(),
                "should be no duplicates after dedup_keep_newest"
            );
            assert_eq!(10, list.len());
            assert_eq!(
                SystemTime::UNIX_EPOCH + Duration::from_secs(100),
                list.iter().map(|e| e.last_updated).min().unwrap(),
                "All old items should be gone"
            );
        }
    }

    mod only_recent {
        use super::*;

        #[test]
        fn do_not_remove_recent() {
            let list: Vec<_> = (0..10).into_iter().map(|i| recent_entry(i)).collect();
            let cache = Cached(list);

            let cache = cache.update(Vec::new()).unwrap();
            assert_eq!(cache.0.len(), 10);
        }
    }

    mod enough_entries {
        use super::*;

        #[test]
        fn remove_old() {
            let list: Vec<_> = (0..5)
                .map(|i| recent_entry(i))
                .chain((5..10).map(|i| old_entry(i, 0)))
                .collect();

            let cache = Cached(list);

            let cache = cache.update(Vec::new()).unwrap();
            assert_eq!(cache.0.len(), 5);
        }
    }

    mod few_entries {
        use super::*;

        #[test]
        fn keep_old() {
            let list: Vec<_> = (0..5)
                .map(|i| recent_entry(i))
                .chain((5..10).map(|i| old_entry(i, 0)))
                .collect();

            let cache = Cached(list);

            let cache = cache.update(Vec::new()).unwrap();
            assert_eq!(cache.0.len(), 5);
        }
    }
}
