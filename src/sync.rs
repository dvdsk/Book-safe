use color_eyre::{
    eyre::{eyre, WrapErr},
    Result,
};
use itertools::{Either, Itertools};
use std::{collections::HashSet, fs, io, net::IpAddr, str::FromStr, thread, time::Duration};
use trust_dns_resolver::error::{ResolveError, ResolveErrorKind};

mod route;

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

fn resolve_sync_routes() -> (HashSet<IpAddr>, Vec<ResolveError>) {
    use trust_dns_resolver::config::*;
    use trust_dns_resolver::Resolver;

    let resolver = Resolver::new(ResolverConfig::default(), ResolverOpts::default()).unwrap();

    let (err, res): (Vec<_>, Vec<_>) = SYNC_BACKENDS
        .into_iter()
        .map(|domain| resolver.lookup_ip(domain))
        .partition_map(Either::from);

    let res: HashSet<_> = res.into_iter().flat_map(|r| r.into_iter()).collect();
    log::debug!("sync routes: {res:?}");
    (res, err)
}

fn routes_from_file() -> Result<Vec<IpAddr>> {
    let text = match fs::read_to_string("routes.txt") {
        Ok(lines) => lines,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => Err(e).wrap_err("Could not \"routes.txt\"")?,
    };

    let routes: Result<Vec<_>, _> = text.lines().map(IpAddr::from_str).collect();
    routes.wrap_err("could not parse adress in file")
}

fn routes_to_file(routes: &HashSet<IpAddr>) -> Result<()> {
    let lines = routes.iter().map(|ip| format!("{ip}")).join("\n");
    fs::write("routes.txt", lines.as_bytes()).wrap_err("Could not cache routes to file")
}

fn sync_routes() -> Result<Vec<IpAddr>> {
    let (mut res, err) = resolve_sync_routes();
    let conn_err = err
        .iter()
        .map(ResolveError::kind)
        .find(|k| matches!(k, ResolveErrorKind::NoConnections));
    if conn_err.is_some() {
        let cached = routes_from_file()?;
        res.extend(cached.iter());
    }

    routes_to_file(&res)?;
    Ok(res.into_iter().collect())
}

/// directly after resuming from sleep the `route` tool does not seem to work
/// therefore this retries `route` a few times
pub fn block() -> Result<()> {
    log::info!("blocking sync");
    let to_block = sync_routes()?;

    let mut attempt = 1;
    let routes = route::table().wrap_err("Error parsing routing table")?;
    for addr in &to_block {
        if routes.contains(addr) {
            continue;
        }

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

    log::debug!("blocked successfull in {attempt} attemp(s)",);
    return Ok(());
}

/// directly after resuming from sleep the `route` tool does not seem to work
/// therefore this retries `route` a few times
pub fn unblock() -> Result<()> {
    log::info!("unblocking sync");
    let to_unblock = routes_from_file().wrap_err("Could not retrieve blocked routes from file")?;

    let mut attempt = 1;
    let routes = route::table().wrap_err("Error parsing routing table")?;
    for addr in &to_unblock {
        if !routes.contains(addr) {
            continue;
        }

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

    log::debug!("unblocked successfull in {attempt} attemp(s)",);
    return Ok(());
}
