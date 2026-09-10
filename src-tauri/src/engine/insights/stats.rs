//! Numeric helpers shared by the insights modules.
//!
//! Every one of these is total: an empty sample yields `None` or `0.0`, never
//! a panic or a NaN. A metrics view is the last place that should be able to
//! take the app down.

/// Mean of the sample, or `None` when there is nothing to average.
pub fn average(values: &[u64]) -> Option<u64> {
    if values.is_empty() {
        return None;
    }
    let total: u128 = values.iter().map(|v| *v as u128).sum();
    Some((total / values.len() as u128) as u64)
}

/// 95th percentile using nearest-rank, so small samples degrade sensibly:
/// one sample returns itself, two return the larger. Never indexes past the
/// end.
pub fn percentile_95(values: &[u64]) -> Option<u64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    // Nearest-rank: ceil(0.95 * n), 1-based, clamped into the slice.
    let rank = ((sorted.len() as f64) * 0.95).ceil() as usize;
    let idx = rank.max(1).min(sorted.len()) - 1;
    Some(sorted[idx])
}

/// `part / whole` in 0.0..=1.0, with zero for an empty denominator.
pub fn rate(part: u32, whole: u32) -> f64 {
    if whole == 0 {
        return 0.0;
    }
    part as f64 / whole as f64
}

/// Percentage change from `previous` to `current`, or `None` when there is no
/// baseline to compare against.
pub fn delta_pct(current: Option<u64>, previous: Option<u64>) -> Option<f64> {
    let (current, previous) = (current?, previous?);
    if previous == 0 {
        return None;
    }
    Some(((current as f64 - previous as f64) / previous as f64) * 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_average_and_percentile_handle_empty_samples() {
        assert_eq!(average(&[]), None);
        assert_eq!(percentile_95(&[]), None);
        assert_eq!(rate(3, 0), 0.0);
        assert_eq!(delta_pct(Some(10), None), None);
        assert_eq!(delta_pct(Some(10), Some(0)), None);
    }

    #[test]
    fn test_percentile_95_degrades_on_tiny_samples() {
        assert_eq!(percentile_95(&[42]), Some(42));
        assert_eq!(percentile_95(&[10, 90]), Some(90));
        assert_eq!(percentile_95(&[30, 10, 20]), Some(30));
    }

    #[test]
    fn test_percentile_95_picks_the_nearest_rank() {
        let values: Vec<u64> = (1..=100).collect();
        assert_eq!(percentile_95(&values), Some(95));
    }

    #[test]
    fn test_average_and_delta_pct() {
        assert_eq!(average(&[10, 20, 30]), Some(20));
        assert_eq!(delta_pct(Some(150), Some(100)), Some(50.0));
        assert_eq!(delta_pct(Some(50), Some(100)), Some(-50.0));
    }
}
