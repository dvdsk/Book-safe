use color_eyre::{
    eyre::{eyre, WrapErr},
    Result,
};
use itertools::{Either, Itertools};
use std::{
    net::IpAddr,
    thread,
    time::{Duration, Instant},
};
use trust_dns_resolver::error::{ResolveError, ResolveErrorKind};

mod cache;
mod route;

use cache::Cached;

const SYNC_BACKENDS: [&str; 9] = [
    "hwr-production-dot-remarkable-production.appspot.com",
    "service-manager-production-dot-remarkable-production.appspot.com",
    "local.appspot.com",
    "my.remarkable.com",
    "ping.remarkable.com",
    "internal.cloud.remarkable.com",
    "ams15s41-in-f20.1e100.net",
    "ams15s48-in-f20.1e100.net",
    "206.137.117.34.bc.googleusercontent.com",
];

fn resolve_sync_routes() -> (Vec<IpAddr>, Vec<ResolveError>) {
    use trust_dns_resolver::config::*;
    use trust_dns_resolver::Resolver;

    let resolver = Resolver::new(ResolverConfig::default(), ResolverOpts::default()).unwrap();

    let (err, res): (Vec<_>, Vec<_>) = SYNC_BACKENDS
        .into_iter()
        .map(|domain| resolver.lookup_ip(domain))
        .partition_map(Either::from);

    // there can be duplicate ips, collecting to hashset deduplicates them
    let mut res: Vec<_> = res.into_iter().flat_map(|r| r.into_iter()).collect();
    res.sort_unstable();
    res.dedup();

    log::debug!("sync routes: {res:?}");
    (res, err)
}

fn sync_routes() -> Result<Vec<IpAddr>> {
    // wifi can take a long time to get up and running
    const TIMEOUT: Duration = Duration::from_secs(30);

    let cache = Cached::load().wrap_err("Could not load files from cache file")?;

    let start = Instant::now();
    let resolved = loop {
        let (resolved, err) = resolve_sync_routes();
        let conn_errs = err
            .iter()
            .map(ResolveError::kind)
            .filter(|k| matches!(k, ResolveErrorKind::NoConnections))
            .count();

        if conn_errs == 0 {
            break resolved;
        }

        if start.elapsed() > TIMEOUT {
            log::warn!("Could not resolve routes within timeout: {TIMEOUT:?}");
            break Vec::new();
        }

        log::debug!("Could not resolve sync adresses, retrying...");
        thread::sleep(Duration::from_millis(200));
    };

    let routes = cache
        .update(resolved)
        .ok_or_else(|| eyre!("cache empty and no routes resolved in time"))?;
    routes
        .cache()
        .wrap_err("Could not cache syn routes to durable storage")?;

    Ok(routes.into_ips())
}

/// directly after resuming from sleep the `route` tool does not seem to work
/// therefore this retries `route` a few times
pub fn block() -> Result<()> {
    log::info!("blocking sync");
    let to_block = sync_routes()?;

    #[cfg(target_arch = "arm")]
    let mut attempt = 1;
    let routes = route::table().wrap_err("Error parsing routing table")?;
    for addr in &to_block {
        if routes.contains(addr) {
            continue;
        }

        #[cfg(not(target_arch = "arm"))]
        log::warn!("not running on a remarkable, skipping block");
        #[cfg(target_arch = "arm")]
        loop {
            match route::block(addr) {
                Err(route::Error::NoEffect) if attempt > 4 => {
                    return Err(eyre!("Timed out blocking"))
                }
                Err(route::Error::NoEffect) => {
                    attempt += 1;
                    thread::sleep(Duration::from_millis(200))
                }
                Err(other) => return Err(other).wrap_err("could not block route"),
                Ok(()) => (),
            }
        }
    }

    #[cfg(target_arch = "arm")]
    log::debug!("blocked successfull in {attempt} attemp(s)",);
    Ok(())
}

/// directly after resuming from sleep the `route` tool does not seem to work
/// therefore this retries `route` a few times
pub fn unblock() -> Result<()> {
    log::info!("unblocking sync");
    let to_unblock = Cached::load().wrap_err("Could not retrieve blocked routes from file")?;

    #[cfg(target_arch = "arm")]
    let mut attempt = 1;
    let routes = route::table().wrap_err("Error parsing routing table")?;
    for addr in &to_unblock.blocked_ips() {
        if !routes.contains(addr) {
            continue;
        }

        #[cfg(not(target_arch = "arm"))]
        log::warn!("not running on a remarkable, skipping unblock");
        #[cfg(target_arch = "arm")]
        loop {
            match route::unblock(addr) {
                Err(route::Error::NoEffect) if attempt > 4 => {
                    return Err(eyre!("Timed out unblocking"))
                }
                Err(route::Error::NoEffect) => {
                    attempt += 1;
                    thread::sleep(Duration::from_millis(200))
                }
                Err(other) => return Err(other).wrap_err("could not unblock route"),
                Ok(()) => (),
            }
        }
    }

    #[cfg(target_arch = "arm")]
    log::debug!("unblocked successfull in {attempt} attemp(s)",);
    Ok(())
}
