use color_eyre::{
    eyre::{eyre, WrapErr},
    Result, Section, SectionExt,
};
use std::{
    collections::HashSet,
    net::IpAddr,
    process::{Command, Output},
};

fn handle_any_error(output: Output, address: IpAddr, text: &'static str) -> Result<()> {
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        Err(eyre!(text)
            .with_section(move || stdout.trim().to_string().header("Stdout:"))
            .with_section(move || stderr.trim().to_string().header("Stderr:"))
            .with_section(move || format!("adress: {address}")))
    } else {
        Ok(())
    }
}

fn block_route(address: IpAddr) -> Result<()> {
    log::debug!("blocking: {address}");
    let output = Command::new("route")
        .arg("add")
        .arg("-host")
        .arg(address.to_string())
        .arg("reject")
        .output()
        .wrap_err("Could not run route")?;
    handle_any_error(output, address, "Command route add returned an error")
}

fn unblock_route(address: IpAddr) -> Result<()> {
    log::debug!("unblocking: {address}");
    let output = Command::new("route")
        .arg("delete")
        .arg("-host")
        .arg(address.to_string())
        .arg("reject")
        .output()
        .wrap_err("Could not run route")?;
    handle_any_error(output, address, "Command route delete returned an error")
}

fn parse_routes() -> Result<HashSet<IpAddr>> {
    let output = Command::new("route")
        .arg("-n")
        .output()
        .wrap_err("Could not run route")?;

    use std::str::FromStr;
    let output = String::from_utf8_lossy(&output.stdout);
    let routes: Result<HashSet<IpAddr>, _> = output
        .lines()
        .skip(2)
        .map(|f| f.split_once(" ").unwrap().0)
        .map(IpAddr::from_str)
        .collect();
    log::debug!("parsed routes: {routes:?}");
    routes.wrap_err("Could not parse routing table entries")
}

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

fn routes() -> Vec<IpAddr> {
    use trust_dns_resolver::config::*;
    use trust_dns_resolver::Resolver;

    let resolver = Resolver::new(ResolverConfig::default(), ResolverOpts::default()).unwrap();

    let mut res: Vec<_> = SYNC_BACKENDS
        .into_iter()
        .map(|domain| resolver.lookup_ip(domain))
        .filter_map(Result::ok)
        .map(|r| r.into_iter())
        .flatten()
        .collect();
    res.sort_unstable();
    res.dedup();
    log::debug!("sync routes: {res:?}");
    res
}

pub fn block() -> Result<()> {
    log::info!("blocking sync");
    let existing = parse_routes().wrap_err("Error parsing routing table")?;
    for addr in routes() {
        if existing.contains(&addr) {
            continue;
        }

        // TODO enable when ip support for is_global lands
        // if !addr.is_global() {
        //     return Err(
        //         eyre!("Tried to block local adress").with_note(|| format!("adress: {addr}"))
        //     );
        // }

        block_route(addr)?;
    }
    Ok(())
}

pub fn unblock() -> Result<()> {
    log::info!("unblocking sync");
    let existing = parse_routes().wrap_err("Error parsing routing table")?;
    for addr in routes() {
        if existing.contains(&addr) {
            unblock_route(addr)?;
        }
    }
    Ok(())
}
