use std::time::Duration;

use surge_ping::{Client, Config, PingIdentifier, PingSequence, ICMP};
use tokio::runtime::{Builder, Runtime};

use crate::domain::Region;

const PING_COUNT: usize = 3;
const PING_TIMEOUT_SECS: u64 = 2;

pub struct PingMonitor {
    runtime: Runtime,
}

impl PingMonitor {
    pub fn new() -> Option<Self> {
        let runtime = Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()
            .ok()?;

        Some(Self { runtime })
    }

    pub fn measure(&self, region: Region) -> Option<Duration> {
        self.runtime.block_on(measure_region(region))
    }
}

async fn measure_region(region: Region) -> Option<Duration> {
    let host = region.ping_host();
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

    let mut pings = Vec::with_capacity(PING_COUNT);
    for i in 0..PING_COUNT {
        match pinger.ping(PingSequence(i as u16), &[0; 56]).await {
            Ok((_, duration)) => {
                eprintln!("[ping] {} -> {:?}", host, duration);
                pings.push(duration);
            }
            Err(err) => {
                eprintln!("[ping] {} -> error: {}", host, err);
            }
        }
    }

    let average = average_duration(&pings)?;
    eprintln!("[ping] {} -> avg: {:?}", host, average);
    Some(average)
}

fn average_duration(samples: &[Duration]) -> Option<Duration> {
    if samples.is_empty() {
        return None;
    }

    let sum: Duration = samples.iter().copied().sum();
    Some(sum / samples.len() as u32)
}

#[cfg(test)]
mod tests {
    use super::average_duration;
    use std::time::Duration;

    mod average_duration_fn {
        use super::{average_duration, Duration};

        #[test]
        fn should_return_none_when_no_samples_are_available() {
            assert_eq!(average_duration(&[]), None);
        }

        #[test]
        fn should_return_the_mean_duration_when_samples_exist() {
            let samples = [
                Duration::from_millis(30),
                Duration::from_millis(60),
                Duration::from_millis(90),
            ];

            assert_eq!(average_duration(&samples), Some(Duration::from_millis(60)));
        }
    }
}
