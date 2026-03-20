use std::time::Duration;

use surge_ping::{Client, Config, PingIdentifier, PingSequence, ICMP};
use tokio::runtime::{Builder, Runtime};
use tokio::time::sleep;

use crate::domain::Region;
use crate::logln;

const PING_SAMPLE_COUNT: usize = 30;
const PING_TIMEOUT_SECS: u64 = 2;
const PING_SAMPLE_DELAY_MS: u64 = 500;
const PING_UI_UPDATE_INTERVAL: u32 = 5;

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

    pub fn sample_average<F>(&self, region: Region, on_update: F) -> Option<Duration>
    where
        F: FnMut(Duration),
    {
        self.runtime.block_on(measure_region(region, on_update))
    }
}

async fn measure_region<F>(region: Region, mut on_update: F) -> Option<Duration>
where
    F: FnMut(Duration),
{
    let host = region.ping_host();
    let ip = tokio::net::lookup_host(format!("{host}:0"))
        .await
        .ok()?
        .next()?
        .ip();

    logln!("[ping] Resolved {} -> {}", host, ip);

    let mut config_builder = Config::builder();
    if ip.is_ipv6() {
        config_builder = config_builder.kind(ICMP::V6);
    }
    let config = config_builder.build();
    let client = Client::new(&config).ok()?;
    let mut pinger = client.pinger(ip, PingIdentifier(0)).await;
    pinger.timeout(Duration::from_secs(PING_TIMEOUT_SECS));

    let mut average = RunningAverage::default();
    for i in 0..PING_SAMPLE_COUNT {
        match pinger.ping(PingSequence(i as u16), &[0; 56]).await {
            Ok((_, duration)) => {
                let current_average = average.push(duration);
                logln!(
                    "[ping] {} -> sample {}/{}: {:?}, avg: {:?}",
                    host,
                    i + 1,
                    PING_SAMPLE_COUNT,
                    duration,
                    current_average
                );
                if should_publish_running_average(average.sample_count()) {
                    on_update(current_average);
                }
            }
            Err(err) => {
                logln!("[ping] {} -> error: {}", host, err);
            }
        }

        if i + 1 < PING_SAMPLE_COUNT {
            sleep(Duration::from_millis(PING_SAMPLE_DELAY_MS)).await;
        }
    }

    let final_average = average.average()?;
    logln!(
        "[ping] {} -> avg from {}/{} samples: {:?}",
        host,
        average.sample_count(),
        PING_SAMPLE_COUNT,
        final_average
    );
    Some(final_average)
}

#[derive(Debug, Default)]
struct RunningAverage {
    total: Duration,
    samples: u32,
}

impl RunningAverage {
    fn push(&mut self, sample: Duration) -> Duration {
        self.total += sample;
        self.samples += 1;
        self.average()
            .expect("running average should exist after at least one sample")
    }

    fn average(&self) -> Option<Duration> {
        if self.samples == 0 {
            return None;
        }

        Some(self.total / self.samples)
    }

    fn sample_count(&self) -> u32 {
        self.samples
    }
}

fn should_publish_running_average(successful_samples: u32) -> bool {
    successful_samples != 0 && successful_samples % PING_UI_UPDATE_INTERVAL == 0
}

#[cfg(test)]
mod tests {
    use super::{should_publish_running_average, RunningAverage};
    use std::time::Duration;

    mod running_average {
        use super::{Duration, RunningAverage};

        #[test]
        fn should_return_none_when_no_samples_are_available() {
            assert_eq!(RunningAverage::default().average(), None);
        }

        #[test]
        fn should_return_the_mean_duration_when_samples_exist() {
            let mut average = RunningAverage::default();
            average.push(Duration::from_millis(30));
            average.push(Duration::from_millis(60));
            average.push(Duration::from_millis(90));

            assert_eq!(average.average(), Some(Duration::from_millis(60)));
        }

        #[test]
        fn push_should_return_the_updated_average() {
            let mut average = RunningAverage::default();

            assert_eq!(
                average.push(Duration::from_millis(40)),
                Duration::from_millis(40)
            );
            assert_eq!(
                average.push(Duration::from_millis(80)),
                Duration::from_millis(60)
            );
        }
    }

    mod should_publish_running_average_fn {
        use super::should_publish_running_average;

        #[test]
        fn should_return_false_before_a_batch_is_complete() {
            assert!(!should_publish_running_average(4));
        }

        #[test]
        fn should_return_true_when_a_batch_boundary_is_reached() {
            assert!(should_publish_running_average(5));
        }
    }
}
