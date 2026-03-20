use std::time::Duration;

use surge_ping::{Client, Config, PingIdentifier, PingSequence, ICMP};
use tokio::runtime::Runtime;

use crate::domain::Region;

const PING_COUNT: usize = 3;
const PING_TIMEOUT_SECS: u64 = 2;

pub fn measure(region: Region) -> Option<Duration> {
    let host = region.ping_host();
    let rt = Runtime::new().ok()?;

    rt.block_on(async {
        let ip = tokio::net::lookup_host(format!("{host}:0"))
            .await
            .ok()?
            .next()?
            .ip();

        eprintln!("[ping] Resolved {} -> {}", host, ip);

        let mut config_builder = Config::builder();
        if ip.is_ipv6() {
            config_builder = config_builder.kind(ICMP::V6);
        }
        let config = config_builder.build();
        let client = Client::new(&config).ok()?;
        let mut pinger = client.pinger(ip, PingIdentifier(0)).await;
        pinger.timeout(Duration::from_secs(PING_TIMEOUT_SECS));

        let mut pings: Vec<Duration> = Vec::with_capacity(PING_COUNT);

        for i in 0..PING_COUNT {
            match pinger.ping(PingSequence(i as u16), &[0; 56]).await {
                Ok((_, dur)) => {
                    eprintln!("[ping] {} -> {:?}", host, dur);
                    pings.push(dur)
                }
                Err(e) => {
                    eprintln!("[ping] {} -> error: {}", host, e);
                    continue;
                }
            }
        }

        if pings.is_empty() {
            eprintln!("[ping] {} -> all requests failed", host);
            return None;
        }

        let sum: Duration = pings.iter().sum();
        let avg = sum / pings.len() as u32;
        eprintln!("[ping] {} -> avg: {:?}", host, avg);
        Some(avg)
    })
}
