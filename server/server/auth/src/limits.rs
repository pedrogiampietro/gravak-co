//! How often one address may try to log in or register.

use std::collections::{HashMap, VecDeque};
use std::net::IpAddr;
use std::time::{Duration, Instant};

/// Attempts allowed per address within [`WINDOW`].
pub const ATTEMPTS: usize = 10;
pub const WINDOW: Duration = Duration::from_secs(60);

/// The recent attempts of each address. Connections from this machine are
/// not counted: the development tools log in many times a minute.
#[derive(Debug, Default)]
pub struct LoginLimiter {
    attempts: HashMap<IpAddr, VecDeque<Instant>>,
}

impl LoginLimiter {
    /// Records an attempt from `ip` at `now`; false when it is one too many.
    pub fn allow(&mut self, ip: IpAddr, now: Instant) -> bool {
        if ip.is_loopback() {
            return true;
        }
        if self.attempts.len() > 10_000 {
            self.forget_before(now);
        }
        let recent = self.attempts.entry(ip).or_default();
        while recent.front().is_some_and(|at| now.duration_since(*at) >= WINDOW) {
            recent.pop_front();
        }
        if recent.len() >= ATTEMPTS {
            return false;
        }
        recent.push_back(now);
        true
    }

    /// Drops the addresses with no attempt in the window.
    fn forget_before(&mut self, now: Instant) {
        self.attempts
            .retain(|_, recent| recent.back().is_some_and(|at| now.duration_since(*at) < WINDOW));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_address_gets_ten_attempts_a_minute() {
        let mut limiter = LoginLimiter::default();
        let ip: IpAddr = [203, 0, 113, 7].into();
        let start = Instant::now();
        for _ in 0..ATTEMPTS {
            assert!(limiter.allow(ip, start));
        }
        assert!(!limiter.allow(ip, start + Duration::from_secs(30)));
        // Another address is not affected.
        assert!(limiter.allow([203, 0, 113, 8].into(), start));
        // A minute after the first attempts, there is room again.
        assert!(limiter.allow(ip, start + WINDOW));
    }

    #[test]
    fn this_machine_is_not_limited() {
        let mut limiter = LoginLimiter::default();
        let start = Instant::now();
        for _ in 0..100 {
            assert!(limiter.allow([127, 0, 0, 1].into(), start));
        }
    }
}
