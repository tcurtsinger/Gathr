//! Ids, error references and clock helpers.

use std::time::{SystemTime, UNIX_EPOCH};

fn random<const N: usize>() -> [u8; N] {
    let mut bytes = [0; N];
    getrandom::fill(&mut bytes).expect("the system's random source failed");
    bytes
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// An OpenTelemetry trace id: 32 lowercase hex characters.
pub fn trace_id() -> String {
    hex(&random::<16>())
}

/// An OpenTelemetry span id: 16 lowercase hex characters.
pub fn span_id() -> String {
    hex(&random::<8>())
}

/// The service's API token.
pub fn token() -> String {
    hex(&random::<32>())
}

/// A random (version 4) UUID, the form Claude Code takes for `--session-id`.
pub fn uuid() -> String {
    let mut b = random::<16>();
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h = hex(&b);
    format!(
        "{}-{}-{}-{}-{}",
        &h[..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..]
    )
}

/// What the person sees next to an error: the first 8 characters of its trace id (0019).
pub fn reference(trace_id: &str) -> &str {
    &trace_id[..8.min(trace_id.len())]
}

/// A ULID for `now_us`: 48 bits of milliseconds then 80 random bits, in Crockford base32.
pub fn ulid(now_us: i64) -> String {
    const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
    let mut low = [0u8; 16];
    low[6..].copy_from_slice(&random::<10>());
    let millis = (now_us.max(0) / 1000) as u128 & ((1 << 48) - 1);
    let value = (millis << 80) | u128::from_be_bytes(low);
    (0..26)
        .map(|i| ALPHABET[((value >> (125 - 5 * i)) & 31) as usize] as char)
        .collect()
}

/// Microseconds since the Unix epoch.
pub fn now_us() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_micros() as i64)
}

/// `us` as UTC, `2026-11-07T09:30:00.123Z`.
pub fn utc(us: i64) -> String {
    let secs = us.div_euclid(1_000_000);
    let millis = us.rem_euclid(1_000_000) / 1000;
    let (days, day_secs) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    let (h, m, s) = (day_secs / 3600, day_secs % 3600 / 60, day_secs % 60);
    format!("{year:04}-{month:02}-{day:02}T{h:02}:{m:02}:{s:02}.{millis:03}Z")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_formats_known_instants() {
        assert_eq!(utc(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(utc(1_000_000_000_123_000), "2001-09-09T01:46:40.123Z");
        assert_eq!(utc(951_782_400_000_000), "2000-02-29T00:00:00.000Z");
    }

    #[test]
    fn ulids_sort_by_time() {
        let earlier = ulid(1_762_500_000_000_000);
        let later = ulid(1_762_500_000_001_000);
        assert!(earlier < later, "{earlier} should sort before {later}");
        assert_eq!(earlier.len(), 26);
    }
}
