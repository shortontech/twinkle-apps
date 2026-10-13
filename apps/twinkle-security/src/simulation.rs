//! Generator v1. Integer counts are requests in a 100 ms bucket, never rates.
//! Independent SplitMix64 finalization streams are keyed by seed/entity/tick/channel.
//! Signed ticks are encoded by their two's-complement u64 representation. Only the
//! mixer wraps; all model arithmetic uses checked operations or proven small bounds.
use sha2::{Digest, Sha256};
use std::collections::VecDeque;

pub const GENERATOR_VERSION: u32 = 1;
pub const CATEGORIES: [&str; 6] = [
    "Credential abuse",
    "Scanning",
    "Injection",
    "Bot abuse",
    "Suspicious egress",
    "Exploit attempts",
];
pub const OUTCOMES: [&str; 3] = ["Allowed", "Challenged", "Blocked"];
pub const TIERS: [&str; 3] = ["Edge", "Application", "Data"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    Standard,
    Dense,
    Stress,
}
impl Preset {
    pub fn name(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Dense => "dense",
            Self::Stress => "stress",
        }
    }
    pub fn services(self) -> usize {
        match self {
            Self::Standard => 60,
            Self::Dense => 240,
            Self::Stress => 1000,
        }
    }
    pub fn sources(self) -> usize {
        match self {
            Self::Standard => 200,
            Self::Dense => 1000,
            Self::Stress => 5000,
        }
    }
    pub fn group_size(self) -> usize {
        match self {
            Self::Standard => 1,
            Self::Dense => 2,
            Self::Stress => 5,
        }
    }
    pub fn groups(self) -> usize {
        self.services() / self.group_size()
    }
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "standard" => Ok(Self::Standard),
            "dense" => Ok(Self::Dense),
            "stress" => Ok(Self::Stress),
            _ => Err(format!("unknown preset: {s}")),
        }
    }
}

/// SplitMix64's published integer finalizer, with independently mixed tuple fields.
/// This is simulation noise, not cryptographic randomness.
pub fn mix(mut x: u64) -> u64 {
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
    x ^ (x >> 31)
}
pub fn random(seed: u64, entity: u64, tick: i64, channel: u64) -> u64 {
    mix(seed
        ^ mix(entity.wrapping_add(0x9e3779b97f4a7c15))
        ^ mix(tick as u64)
        ^ mix(channel.wrapping_add(0xd1b54a32d192ed03)))
}
pub fn phase(tick: i64) -> &'static str {
    match tick.rem_euclid(3000) / 600 {
        0 => "Baseline",
        1 => "Legitimate surge",
        2 => "Credential campaign",
        3 => "Distributed scanning",
        _ => "Mitigation / recovery",
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Service {
    pub id: usize,
    pub capacity: u64, // requests/sec; also fixed chart domain metadata
    pub inventory: u64,
    pub requests: u64,
    pub suspicious: [u64; 6],
    pub outcomes: [u64; 3],
    pub score_milli: u64, // 0..100000; floor rounding at every smoothing step
    pub hosts: u64,
}
impl Service {
    pub fn label(&self) -> String {
        format!(
            "{}-{:04}",
            if self.id.is_multiple_of(6) {
                "login"
            } else {
                "svc"
            },
            self.id
        )
    }
    pub fn tier(&self) -> usize {
        (self.id / 6) % 3
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    pub id: usize,
    pub service: usize,
    pub category: usize,
    pub bucket_count: u64,
    pub rolling_count: u64,
}
impl Source {
    pub fn label(&self) -> String {
        format!("source-{:05}", self.id)
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeatBucket {
    pub tick: i64,
    pub groups: Vec<[u64; 6]>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrafficBucket {
    pub tick: i64,
    pub outcomes: [u64; 3],
}

pub struct Simulation {
    pub seed: u64,
    pub preset: Preset,
    pub tick: i64,
    pub services: Vec<Service>,
    pub sources: Vec<Source>,
    pub heat: VecDeque<HeatBucket>,
    pub traffic: VecDeque<TrafficBucket>,
    source_history: VecDeque<Vec<u64>>,
}
impl Simulation {
    /// Initialize before -700, replay [-700,-1], then publish tick zero.
    pub fn new(seed: u64, preset: Preset) -> Self {
        let mut s = Self {
            seed,
            preset,
            tick: -701,
            services: (0..preset.services())
                .map(|id| Service {
                    id,
                    capacity: 3000 + (id as u64 % 11) * 150,
                    inventory: 100 + id as u64 % 101,
                    requests: 0,
                    suspicious: [0; 6],
                    outcomes: [0; 3],
                    score_milli: 0,
                    hosts: 0,
                })
                .collect(),
            sources: (0..preset.sources())
                .map(|id| Source {
                    id,
                    service: 0,
                    category: 0,
                    bucket_count: 0,
                    rolling_count: 0,
                })
                .collect(),
            heat: VecDeque::with_capacity(100),
            traffic: VecDeque::with_capacity(600),
            source_history: VecDeque::with_capacity(100),
        };
        for _ in -700..=0 {
            s.step();
        }
        s
    }
    pub fn step(&mut self) {
        self.tick = self.tick.checked_add(1).expect("simulation tick overflow");
        let t = self.tick.rem_euclid(3000) as u64;
        // Phase envelopes in thousandths. Ramps take 100 ticks; mitigation fades
        // across 600 ticks. Legitimate traffic also returns continuously to baseline.
        let ramp = |start: u64| ((t - start).min(100) * 10).min(1000);
        let credential = if (1200..1800).contains(&t) {
            ramp(1200)
        } else if (1800..1900).contains(&t) {
            (1900 - t) * 10
        } else {
            0
        };
        let scanning = if (1800..2400).contains(&t) {
            ramp(1800)
        } else if t >= 2400 {
            (3000 - t) * 1000 / 600
        } else {
            0
        };
        let surge = if (600..1200).contains(&t) {
            ramp(600)
        } else if (1200..1300).contains(&t) {
            (1300 - t) * 10
        } else {
            0
        };
        for service in &mut self.services {
            service.suspicious = [0; 6];
            // 25–35% of capacity at baseline, up to another 35% in demand surge.
            let noise = random(self.seed, service.id as u64, self.tick, 0) % 100;
            service.requests = service.capacity * (250 + noise + surge * 350 / 1000) / 10_000;
        }
        let expired = if self.source_history.len() == 100 {
            self.source_history.pop_front()
        } else {
            None
        };
        let mut counts = Vec::with_capacity(self.sources.len());
        for source in &mut self.sources {
            let id = source.id as u64;
            let rotation = self.tick.div_euclid(30);
            let r = random(self.seed, id, rotation, 1);
            let credential_source = r % 1000 < credential;
            source.category = if credential_source {
                0
            } else {
                (r % 6) as usize
            };
            source.service = if credential_source {
                (r % self.preset.services().div_ceil(6) as u64) as usize * 6
            } else {
                (r % self.services.len() as u64) as usize
            };
            // Every source has a small background contribution; campaign volume is
            // bounded even for stress. All request outcomes derive from this count.
            let noise = random(self.seed, id, self.tick, 2) % 3;
            source.bucket_count = 1
                + noise
                + if credential_source {
                    credential * (10 + r % 20) / 1000
                } else {
                    0
                }
                + scanning * (5 + r % 12) / 1000;
            source.rolling_count = source
                .rolling_count
                .checked_add(source.bucket_count)
                .unwrap()
                .checked_sub(expired.as_ref().map_or(0, |v| v[source.id]))
                .unwrap();
            counts.push(source.bucket_count);
            let service = &mut self.services[source.service];
            service.suspicious[source.category] = service.suspicious[source.category]
                .checked_add(source.bucket_count)
                .unwrap();
        }
        self.source_history.push_back(counts);
        let mut outcomes = [0_u64; 3];
        let mut groups = vec![[0_u64; 6]; self.preset.groups()];
        for service in &mut self.services {
            let suspicious = service.suspicious.iter().sum::<u64>();
            service.requests = service.requests.checked_add(suspicious).unwrap();
            let mitigation = if t >= 2400 {
                450 + (t - 2400) * 500 / 600
            } else {
                450
            };
            let blocked = suspicious * mitigation / 1000;
            let challenged = (suspicious - blocked) / 2;
            service.outcomes = [service.requests - blocked - challenged, challenged, blocked];
            let target = suspicious * 100_000 / service.requests.max(1);
            service.score_milli = (service.score_milli * 9 + target) / 10;
            service.hosts = service.inventory * service.score_milli / 100_000;
            for c in 0..6 {
                groups[service.id / self.preset.group_size()][c] += service.suspicious[c];
            }
            for (total, value) in outcomes.iter_mut().zip(service.outcomes) {
                *total += value;
            }
        }
        if self.heat.len() == 100 {
            self.heat.pop_front();
        }
        self.heat.push_back(HeatBucket {
            tick: self.tick,
            groups,
        });
        if self.traffic.len() == 600 {
            self.traffic.pop_front();
        }
        self.traffic.push_back(TrafficBucket {
            tick: self.tick,
            outcomes,
        });
    }
    pub fn ranked_sources(&self) -> Vec<&Source> {
        let mut sources = self.sources.iter().collect::<Vec<_>>();
        sources.sort_unstable_by_key(|s| (std::cmp::Reverse(s.rolling_count), s.id));
        sources.truncate(20);
        sources
    }
    pub fn totals(&self) -> (u64, u64, u64) {
        (
            self.services.iter().map(|s| s.requests).sum(),
            self.services.iter().map(|s| s.outcomes[2]).sum(),
            self.services.iter().map(|s| s.hosts).sum(),
        )
    }
    pub fn group_capacity(&self, group: usize) -> u64 {
        self.services[group * self.preset.group_size()..(group + 1) * self.preset.group_size()]
            .iter()
            .map(|s| s.capacity)
            .sum()
    }
    pub fn checksum(&self) -> String {
        // Generator version and preset are part of identity. Every scalar uses u64
        // LE; signed ticks use two's-complement LE. Strings are u64-length prefixed.
        let mut hash = Sha256::new();
        let mut word = |v: u64| hash.update(v.to_le_bytes());
        word(GENERATOR_VERSION as u64);
        word(self.seed);
        word(self.tick as u64);
        word(self.preset.name().len() as u64);
        hash.update(self.preset.name().as_bytes());
        let mut word = |v: u64| hash.update(v.to_le_bytes());
        word(self.services.len() as u64);
        for s in &self.services {
            for v in [
                s.id as u64,
                s.capacity,
                s.inventory,
                s.requests,
                s.score_milli,
                s.hosts,
            ] {
                word(v);
            }
            for v in s.suspicious.into_iter().chain(s.outcomes) {
                word(v);
            }
        }
        word(self.sources.len() as u64);
        for s in &self.sources {
            for v in [
                s.id as u64,
                s.service as u64,
                s.category as u64,
                s.bucket_count,
                s.rolling_count,
            ] {
                word(v);
            }
        }
        word(self.traffic.len() as u64);
        for b in &self.traffic {
            word(b.tick as u64);
            for v in b.outcomes {
                word(v);
            }
        }
        word(self.heat.len() as u64);
        for b in &self.heat {
            word(b.tick as u64);
            word(b.groups.len() as u64);
            for g in &b.groups {
                for v in g {
                    word(*v);
                }
            }
        }
        word(self.source_history.len() as u64);
        for counts in &self.source_history {
            word(counts.len() as u64);
            for v in counts {
                word(*v);
            }
        }
        format!("{:x}", hash.finalize())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canonical_golden_vectors_cover_presets_seeds_and_phase_boundaries() {
        let fixtures: serde_json::Value =
            serde_json::from_str(include_str!("../evidence/generator-v1-golden.json")).unwrap();
        let mut current: Option<Simulation> = None;
        for v in fixtures.as_array().unwrap() {
            let seed = v["seed"].as_u64().unwrap();
            let preset = Preset::parse(v["preset"].as_str().unwrap()).unwrap();
            let tick = v["tick"].as_i64().unwrap();
            if current
                .as_ref()
                .is_none_or(|s| s.seed != seed || s.preset != preset)
            {
                current = Some(Simulation::new(seed, preset));
            }
            let s = current.as_mut().unwrap();
            while s.tick < tick {
                s.step();
            }
            assert_eq!(
                s.checksum(),
                v["checksum"].as_str().unwrap(),
                "seed={seed}, preset={}, tick={tick}",
                preset.name()
            );
        }
    }
    #[test]
    fn mixer_vectors_and_signed_encoding() {
        assert_eq!(mix(0), 0);
        assert_eq!(mix(1), 0x5692161d100b05e5);
        assert_eq!(mix(u64::MAX), 0xb4d055fcf2cbbd7b);
        assert_ne!(random(7, 1, -1, 2), random(7, 1, 1, 2));
        assert_eq!(phase(-1), "Mitigation / recovery");
    }
    #[test]
    fn histories_conserve_counts_and_rolling_sources_match_reference() {
        for preset in [Preset::Standard, Preset::Dense, Preset::Stress] {
            let mut s = Simulation::new(42, preset);
            for _ in 0..110 {
                s.step();
                for service in &s.services {
                    assert_eq!(service.requests, service.outcomes.iter().sum::<u64>());
                    assert!(service.suspicious.iter().sum::<u64>() <= service.requests);
                    assert!(service.score_milli <= 100000 && service.hosts <= service.inventory);
                }
                assert_eq!(s.traffic.len(), 600);
                assert_eq!(s.heat.len(), 100);
                assert_eq!(s.source_history.len(), 100);
                assert_eq!(s.heat.front().unwrap().tick, s.tick - 99);
                for source in &s.sources {
                    assert_eq!(
                        source.rolling_count,
                        s.source_history.iter().map(|v| v[source.id]).sum::<u64>()
                    );
                }
                assert_eq!(
                    s.heat.back().unwrap().groups.iter().flatten().sum::<u64>(),
                    s.sources.iter().map(|v| v.bucket_count).sum::<u64>()
                );
            }
        }
    }
    #[test]
    fn restart_seed_and_boundary_replay() {
        let mut a = Simulation::new(42, Preset::Standard);
        assert_eq!(
            a.checksum(),
            Simulation::new(42, Preset::Standard).checksum()
        );
        assert_ne!(
            a.checksum(),
            Simulation::new(43, Preset::Standard).checksum()
        );
        let mut b = Simulation::new(42, Preset::Standard);
        for _ in 0..3001 {
            a.step();
            b.step();
        }
        assert_eq!(a.checksum(), b.checksum());
        assert_eq!(a.tick, 3001);
        assert_eq!(a.traffic.front().unwrap().tick, 2402);
        assert_eq!(phase(2999), "Mitigation / recovery");
        assert_eq!(phase(3000), "Baseline");
    }
    #[test]
    fn legitimate_surge_and_campaign_have_different_risk() {
        let mut s = Simulation::new(42, Preset::Standard);
        let mut baseline = (0, 0);
        let mut surge = (0, 0);
        let mut attack = (0, 0);
        let mut recovery = (0, 0);
        for _ in 0..2999 {
            s.step();
            if [599, 1199, 1799, 2999].contains(&s.tick) {
                let (requests, _, _) = s.totals();
                let suspicious = s.sources.iter().map(|s| s.bucket_count).sum();
                match s.tick {
                    599 => baseline = (requests, suspicious),
                    1199 => surge = (requests, suspicious),
                    1799 => attack = (requests, suspicious),
                    _ => recovery = (requests, suspicious),
                }
            }
        }
        assert!(surge.0 > baseline.0 * 17 / 10);
        assert!(surge.1 < baseline.1 * 12 / 10);
        assert!(attack.1 > baseline.1 * 5);
        assert!(recovery.1 < attack.1 / 3);
    }
}
