/// What to do with the configured memory, given the highest Max Memory Used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Advice {
    /// Configured memory can come down. `memory_mb` is the suggestion.
    Downsize { memory_mb: u32 },
    /// Max Memory Used is at least 90% of the setting, so the true peak may be higher.
    Raise { memory_mb: u32 },
    /// The suggestion is the current setting, or the setting is already 10240 MB.
    Keep,
}

impl Advice {
    #[must_use]
    pub fn action(self) -> &'static str {
        match self {
            Self::Downsize { .. } => "downsize",
            Self::Raise { .. } => "raise",
            Self::Keep => "keep",
        }
    }

    /// Suggested memory, or the configured memory when the advice is keep.
    #[must_use]
    pub fn memory_mb(self, configured_mb: u32) -> u32 {
        match self {
            Self::Downsize { memory_mb } | Self::Raise { memory_mb } => memory_mb,
            Self::Keep => configured_mb,
        }
    }

    #[must_use]
    pub fn label(self) -> String {
        match self {
            Self::Downsize { memory_mb } => format!("downsize to {memory_mb} MB"),
            Self::Raise { memory_mb } => format!("raise to {memory_mb} MB"),
            Self::Keep => "keep".to_string(),
        }
    }
}

/// `ceil(value * percent / 100)` with `percent` = `100 + headroom`.
fn ceil_percent(value: u32, percent: u32) -> u32 {
    let product = u64::from(value) * u64::from(percent);
    let ceiled = product.div_ceil(100);
    u32::try_from(ceiled).unwrap_or(10_240)
}

/// Headroom is a percent above the base (0..=100). The downsize base is Max
/// Memory Used. The raise base is the configured size, because Max Memory Used
/// cannot show a peak above that size.
#[must_use]
pub fn advise(memory_mb: u32, max_used_mb: u32, headroom_percent: u8) -> Advice {
    let head = u32::from(headroom_percent);
    let saturated = u64::from(max_used_mb) * 100 >= u64::from(memory_mb) * 90;
    if saturated {
        let grown = ceil_percent(memory_mb, 100 + head).min(10_240);
        let target = grown.max(memory_mb.saturating_add(1)).min(10_240);
        if target > memory_mb {
            Advice::Raise { memory_mb: target }
        } else {
            Advice::Keep
        }
    } else {
        let grown = ceil_percent(max_used_mb, 100 + head);
        let target = grown.clamp(128, 10_240);
        if target < memory_mb {
            Advice::Downsize { memory_mb: target }
        } else {
            Advice::Keep
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{advise, Advice};

    #[test]
    fn downsize_floors_at_128() {
        assert_eq!(advise(512, 40, 20), Advice::Downsize { memory_mb: 128 });
        assert_eq!(advise(512, 64, 20), Advice::Downsize { memory_mb: 128 });
    }

    #[test]
    fn downsize_uses_ceil_of_max_used_times_headroom() {
        // 200 * 1.20 = 240 exactly.
        assert_eq!(advise(256, 200, 20), Advice::Downsize { memory_mb: 240 });
    }

    #[test]
    fn keep_when_the_suggestion_is_the_current_size() {
        assert_eq!(advise(128, 20, 20), Advice::Keep);
        assert_eq!(advise(240, 200, 20), Advice::Keep);
    }

    #[test]
    fn raise_starts_at_90_percent_and_uses_configured_memory() {
        assert_eq!(advise(1000, 899, 20), Advice::Keep);
        assert_eq!(advise(1000, 900, 20), Advice::Raise { memory_mb: 1200 });
        // 256 * 1.20 = 307.2, ceil 308. Max used is the cap, so it is not the base.
        // 230/256 is just under 90 percent, and ceil(230 * 1.20) is not below 256.
        assert_eq!(advise(256, 230, 20), Advice::Keep);
        // 231/256 is at least 90 percent. The base is the configured 256, so ceil(256 * 1.20) = 308.
        assert_eq!(advise(256, 231, 20), Advice::Raise { memory_mb: 308 });
        // 117/129 is at least 90 percent, so raise wins over a downsize to 128.
        assert_eq!(advise(129, 117, 5), Advice::Raise { memory_mb: 136 });
    }

    #[test]
    fn saturated_10240_stays() {
        assert_eq!(advise(10_240, 10_000, 20), Advice::Keep);
        assert_eq!(advise(10_240, 100, 20), Advice::Downsize { memory_mb: 128 });
    }

    #[test]
    fn headroom_zero_and_one_hundred() {
        assert_eq!(advise(512, 100, 0), Advice::Downsize { memory_mb: 128 });
        assert_eq!(advise(512, 400, 0), Advice::Downsize { memory_mb: 400 });
        // 100% headroom: 400 * 2 = 800, which is above 512, so keep (not saturated).
        assert_eq!(advise(512, 400, 100), Advice::Keep);
        assert_eq!(advise(128, 128, 0), Advice::Raise { memory_mb: 129 });
    }

    #[test]
    fn max_used_above_configured_still_raises() {
        assert_eq!(advise(128, 200, 20), Advice::Raise { memory_mb: 154 });
    }

    #[test]
    fn labels() {
        assert_eq!(Advice::Keep.action(), "keep");
        assert_eq!(Advice::Keep.label(), "keep");
        assert_eq!(Advice::Keep.memory_mb(512), 512);
        let down = Advice::Downsize { memory_mb: 128 };
        assert_eq!(down.label(), "downsize to 128 MB");
        assert_eq!(down.memory_mb(512), 128);
        let up = Advice::Raise { memory_mb: 308 };
        assert_eq!(up.action(), "raise");
        assert_eq!(up.label(), "raise to 308 MB");
    }
}
