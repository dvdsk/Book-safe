use color_eyre::eyre;
use eyre::{eyre, Result, WrapErr};
use time::Time;

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

