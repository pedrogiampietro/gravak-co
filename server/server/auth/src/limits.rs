//! How often one address may try to log in or register, and how many wrong
//! passwords it may give before it waits.

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

/// Wrong passwords that lock an address out, within [`FAILURE_WINDOW`].
pub const FAILURES: usize = 5;
pub const FAILURE_WINDOW: Duration = Duration::from_secs(10 * 60);
/// How long an address is locked out after [`FAILURES`] wrong passwords.
pub const LOCKOUT: Duration = Duration::from_secs(5 * 60);

/// The wrong passwords of each address, and the addresses locked out for
/// them. A right password forgets an address's wrong ones. Connections from
/// this machine are never locked out.
#[derive(Debug, Default)]
pub struct FailedLogins {
    failures: HashMap<IpAddr, VecDeque<Instant>>,
    locked: HashMap<IpAddr, Instant>,
}

impl FailedLogins {
    /// How much longer `ip` is locked out, None when it is not.
    pub fn locked_for(&mut self, ip: IpAddr, now: Instant) -> Option<Duration> {
        let until = *self.locked.get(&ip)?;
        if now >= until {
            self.locked.remove(&ip);
            return None;
        }
        Some(until - now)
    }

    /// Records a wrong password from `ip`; true when it locks the address out.
    pub fn failed(&mut self, ip: IpAddr, now: Instant) -> bool {
        if ip.is_loopback() {
            return false;
        }
        if self.failures.len() > 10_000 {
            self.failures
                .retain(|_, recent| recent.back().is_some_and(|at| now.duration_since(*at) < FAILURE_WINDOW));
            self.locked.retain(|_, until| *until > now);
        }
        let recent = self.failures.entry(ip).or_default();
        while recent.front().is_some_and(|at| now.duration_since(*at) >= FAILURE_WINDOW) {
            recent.pop_front();
        }
        recent.push_back(now);
        if recent.len() < FAILURES {
            return false;
        }
        self.failures.remove(&ip);
        self.locked.insert(ip, now + LOCKOUT);
        true
    }

    /// A right password from `ip`: its wrong ones are forgotten.
    pub fn succeeded(&mut self, ip: IpAddr) {
        self.failures.remove(&ip);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn five_wrong_passwords_lock_an_address_out_for_a_while() {
        let mut logins = FailedLogins::default();
        let ip: IpAddr = [203, 0, 113, 7].into();
        let start = Instant::now();
        for attempt in 1..FAILURES {
            assert!(!logins.failed(ip, start + Duration::from_secs(attempt as u64)));
            assert_eq!(logins.locked_for(ip, start), None);
        }
        assert!(logins.failed(ip, start + Duration::from_secs(10)), "the fifth locks it out");
        assert_eq!(logins.locked_for(ip, start + Duration::from_secs(70)), Some(LOCKOUT - Duration::from_secs(60)));
        assert_eq!(logins.locked_for([203, 0, 113, 8].into(), start), None, "another address is not affected");
        assert_eq!(logins.locked_for(ip, start + Duration::from_secs(10) + LOCKOUT), None);
    }

    #[test]
    fn a_right_password_or_time_forgets_the_wrong_ones() {
        let mut logins = FailedLogins::default();
        let ip: IpAddr = [203, 0, 113, 7].into();
        let start = Instant::now();
        for _ in 1..FAILURES {
            logins.failed(ip, start);
        }
        logins.succeeded(ip);
        assert!(!logins.failed(ip, start), "counting starts over after a right password");
        for _ in 2..FAILURES {
            logins.failed(ip, start);
        }
        assert!(!logins.failed(ip, start + FAILURE_WINDOW), "the old ones are out of the window");
        for _ in 0..100 {
            assert!(!logins.failed([127, 0, 0, 1].into(), start), "this machine is never locked out");
        }
    }

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
