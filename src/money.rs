use std::cmp::{Ord, Ordering};

use rust_decimal::Decimal;

use crate::error::Error;

/// Largest coefficient of a 96-bit decimal (`2^96 - 1`, 29 digits).
const MAX_COEFF: u128 = (1_u128 << 96) - 1;

/// `memory_mb * billed_ms / 1024 / 1000`. One gibibyte is 1024 MiB, which is
/// the 1 GB the Lambda pricing page uses.
///
/// The quotient is exact integer arithmetic. It is stored only when it is an
/// exact 96-bit decimal.
///
/// # Errors
/// Returns "price overflow" when the quotient is not an exact decimal.
pub fn gb_seconds(memory_mb: u32, billed_ms: u64) -> Result<Decimal, Error> {
    exact_decimal(&exact_gb(memory_mb, billed_ms))
}

/// `memory_mb * billed_ms / 1024 / 1000`, unrounded.
pub(crate) fn exact_gb(memory_mb: u32, billed_ms: u64) -> Ratio {
    Ratio {
        num: Big::from_u64(u64::from(memory_mb)).mul_u64(billed_ms),
        den: Big::from_u64(1_024_000),
    }
    .reduced()
}

/// `gb_seconds * rate`, unrounded. `rate` is a non-negative decimal.
pub(crate) fn exact_duration(memory_mb: u32, billed_ms: u64, rate: Decimal) -> Ratio {
    exact_gb(memory_mb, billed_ms).mul_decimal(rate)
}

/// `requests * per_million / 1_000_000`, unrounded.
pub(crate) fn exact_request(requests: u64, per_million: Decimal) -> Ratio {
    Ratio {
        num: Big::from_u64(requests),
        den: Big::from_u64(1),
    }
    .mul_decimal(per_million)
    .div_u64(1_000_000)
}

/// The rational as a 96-bit decimal: scale 0 through 28, coefficient at most
/// `2^96 - 1`. Nothing is rounded. A value that is not already that decimal
/// is an error.
///
/// # Errors
/// Returns "price overflow" when the value is not an exact decimal of that shape.
pub(crate) fn exact_decimal(ratio: &Ratio) -> Result<Decimal, Error> {
    if ratio.num.is_zero() {
        return Ok(Decimal::ZERO);
    }
    let reduced = ratio.clone().reduced();
    if reduced.den.is_zero() {
        return Err(money_error("price overflow"));
    }
    let (den, twos) = strip_factor(&reduced.den, 2);
    let (den, fives) = strip_factor(&den, 5);
    if !den.is_one() {
        return Err(money_error("price overflow"));
    }
    let scale = twos.max(fives);
    if scale > 28 {
        return Err(money_error("price overflow"));
    }
    // num / (2^twos * 5^fives) = coeff / 10^scale.
    let mut coeff = reduced.num;
    for _ in 0..(scale - twos) {
        coeff = coeff.mul_u64(2);
    }
    for _ in 0..(scale - fives) {
        coeff = coeff.mul_u64(5);
    }
    let Some(coeff) = coeff.to_u128() else {
        return Err(money_error("price overflow"));
    };
    if coeff > MAX_COEFF {
        return Err(money_error("price overflow"));
    }
    let (coeff, scale) = strip_trailing_zeros(coeff, scale);
    Decimal::try_from_i128_with_scale(coeff as i128, scale)
        .map_err(|_| money_error("price overflow"))
}

/// Divide `value` by `factor` while it divides evenly. Returns the leftover
/// and how many times `factor` divided it.
fn strip_factor(value: &Big, factor: u64) -> (Big, u32) {
    let divisor = Big::from_u64(factor);
    let mut left = value.clone();
    let mut count = 0_u32;
    loop {
        let (quot, rem) = left.divmod(&divisor);
        if !rem.is_zero() {
            return (left, count);
        }
        left = quot;
        count += 1;
    }
}

fn strip_trailing_zeros(mut coeff: u128, mut scale: u32) -> (u128, u32) {
    while scale > 0 && coeff % 10 == 0 {
        coeff /= 10;
        scale -= 1;
    }
    (coeff, scale)
}

/// A non-negative rational `num / den` with `den > 0`.
#[derive(Clone)]
pub(crate) struct Ratio {
    num: Big,
    den: Big,
}

impl Ratio {
    pub(crate) fn zero() -> Self {
        Self {
            num: Big::zero(),
            den: Big::from_u64(1),
        }
    }

    pub(crate) fn add(self, other: Self) -> Self {
        let num = self.num.mul(&other.den).add(&other.num.mul(&self.den));
        let den = self.den.mul(&other.den);
        Self { num, den }.reduced()
    }

    fn mul_decimal(self, value: Decimal) -> Self {
        let mantissa = value.mantissa();
        if mantissa < 0 {
            // A negative rate is rejected at the price file. This sentinel
            // is not an exact decimal, so the run is an error rather than a
            // made-up amount.
            return Self {
                num: Big::from_u128(MAX_COEFF).add_u64(1),
                den: Big::from_u64(1),
            };
        }
        let scale = value.scale();
        Self {
            num: self.num.mul(&Big::from_u128(mantissa as u128)),
            den: self.den.mul(&Big::pow10(scale)),
        }
    }

    fn div_u64(self, divisor: u64) -> Self {
        Self {
            num: self.num,
            den: self.den.mul_u64(divisor),
        }
    }

    fn reduced(self) -> Self {
        if self.num.is_zero() {
            return Self::zero();
        }
        let factor = gcd(self.num.clone(), self.den.clone());
        Self {
            num: self.num.divmod(&factor).0,
            den: self.den.divmod(&factor).0,
        }
    }
}

/// Little-endian base-2^64 integer. Empty means zero.
#[derive(Clone)]
struct Big {
    limbs: Vec<u64>,
}

impl Big {
    fn zero() -> Self {
        Self { limbs: Vec::new() }
    }

    fn from_u64(value: u64) -> Self {
        if value == 0 {
            Self::zero()
        } else {
            Self { limbs: vec![value] }
        }
    }

    fn from_u128(value: u128) -> Self {
        if value == 0 {
            Self::zero()
        } else if value <= u128::from(u64::MAX) {
            Self::from_u64(value as u64)
        } else {
            Self {
                limbs: vec![value as u64, (value >> 64) as u64],
            }
        }
    }

    fn is_zero(&self) -> bool {
        self.limbs.is_empty()
    }

    fn is_one(&self) -> bool {
        matches!(self.limbs.as_slice(), [1])
    }

    fn to_u128(&self) -> Option<u128> {
        match *self.limbs.as_slice() {
            [] => Some(0),
            [lo] => Some(u128::from(lo)),
            [lo, hi] => Some(u128::from(lo) | (u128::from(hi) << 64)),
            _ => None,
        }
    }

    fn add(&self, other: &Self) -> Self {
        let width = self.limbs.len().max(other.limbs.len());
        let mut limbs = Vec::with_capacity(width + 1);
        let mut carry = 0_u128;
        for index in 0..width {
            let left = u128::from(*self.limbs.get(index).unwrap_or(&0));
            let right = u128::from(*other.limbs.get(index).unwrap_or(&0));
            let sum = left + right + carry;
            limbs.push(sum as u64);
            carry = sum >> 64;
        }
        if carry != 0 {
            limbs.push(carry as u64);
        }
        Self { limbs }
    }

    fn add_u64(&self, value: u64) -> Self {
        self.add(&Self::from_u64(value))
    }

    fn mul_u64(&self, factor: u64) -> Self {
        if factor == 0 || self.is_zero() {
            return Self::zero();
        }
        let mut limbs = Vec::with_capacity(self.limbs.len() + 1);
        let mut carry = 0_u128;
        for limb in &self.limbs {
            let product = u128::from(*limb) * u128::from(factor) + carry;
            limbs.push(product as u64);
            carry = product >> 64;
        }
        if carry != 0 {
            limbs.push(carry as u64);
        }
        Self { limbs }
    }

    fn mul(&self, other: &Self) -> Self {
        if self.is_zero() || other.is_zero() {
            return Self::zero();
        }
        let mut limbs = vec![0_u64; self.limbs.len() + other.limbs.len()];
        for (index, left) in self.limbs.iter().enumerate() {
            let mut carry = 0_u128;
            for (offset, right) in other.limbs.iter().enumerate() {
                let product = u128::from(*left) * u128::from(*right)
                    + u128::from(limbs[index + offset])
                    + carry;
                limbs[index + offset] = product as u64;
                carry = product >> 64;
            }
            limbs[index + other.limbs.len()] = carry as u64;
        }
        Self { limbs }.normalized()
    }

    fn pow10(exp: u32) -> Self {
        let mut value = Self::from_u64(1);
        for _ in 0..exp {
            value = value.mul_u64(10);
        }
        value
    }

    /// Quotient and remainder, exact. `den` is not zero (a zero divisor
    /// returns a zero quotient and `self`, and no caller passes one).
    ///
    /// Native `u128` when both sides fit, short division by one limb, and
    /// Knuth's algorithm D (base 2^64) otherwise. No allocation per bit.
    fn divmod(&self, den: &Self) -> (Self, Self) {
        if den.is_zero() || self.cmp(den) == Ordering::Less {
            return (Self::zero(), self.clone());
        }
        if let (Some(num), Some(div)) = (self.to_u128(), den.to_u128()) {
            return (Self::from_u128(num / div), Self::from_u128(num % div));
        }
        if let [div] = *den.limbs.as_slice() {
            let div = u128::from(div);
            let mut quot = vec![0_u64; self.limbs.len()];
            let mut rem = 0_u128;
            for (index, limb) in self.limbs.iter().enumerate().rev() {
                let cur = (rem << 64) | u128::from(*limb);
                quot[index] = (cur / div) as u64;
                rem = cur % div;
            }
            return (Self { limbs: quot }.normalized(), Self::from_u128(rem));
        }
        self.knuth_divmod(den)
    }

    /// Knuth, The Art of Computer Programming 4.3.1, algorithm D. The divisor
    /// has at least two limbs and is not larger than `self`.
    fn knuth_divmod(&self, den: &Self) -> (Self, Self) {
        let n = den.limbs.len();
        let m = self.limbs.len() - n;
        let shift = den.limbs[n - 1].leading_zeros();
        let divisor = shl_limbs(&den.limbs, shift, false);
        let mut rem = shl_limbs(&self.limbs, shift, true);
        let mut quot = vec![0_u64; m + 1];
        let top = u128::from(divisor[n - 1]);
        let second = u128::from(divisor[n - 2]);
        for j in (0..=m).rev() {
            let head = (u128::from(rem[j + n]) << 64) | u128::from(rem[j + n - 1]);
            let mut qhat = head / top;
            let mut rhat = head % top;
            while qhat >> 64 != 0 || qhat * second > ((rhat << 64) | u128::from(rem[j + n - 2])) {
                qhat -= 1;
                rhat += top;
                if rhat >> 64 != 0 {
                    break;
                }
            }
            // rem[j..=j+n] -= qhat * divisor
            let mut mul_carry = 0_u128;
            let mut borrow = false;
            for i in 0..n {
                let product = qhat * u128::from(divisor[i]) + mul_carry;
                mul_carry = product >> 64;
                let (diff, b1) = rem[i + j].overflowing_sub(product as u64);
                let (diff, b2) = diff.overflowing_sub(u64::from(borrow));
                rem[i + j] = diff;
                borrow = b1 | b2;
            }
            let (diff, b1) = rem[j + n].overflowing_sub(mul_carry as u64);
            let (diff, b2) = diff.overflowing_sub(u64::from(borrow));
            rem[j + n] = diff;
            if b1 | b2 {
                // qhat was one too large: add the divisor back.
                qhat -= 1;
                let mut carry = 0_u128;
                for i in 0..n {
                    let sum = u128::from(rem[i + j]) + u128::from(divisor[i]) + carry;
                    rem[i + j] = sum as u64;
                    carry = sum >> 64;
                }
                rem[j + n] = rem[j + n].wrapping_add(carry as u64);
            }
            quot[j] = qhat as u64;
        }
        rem.truncate(n);
        let rem = shr_limbs(&rem, shift);
        (Self { limbs: quot }.normalized(), rem.normalized())
    }

    fn normalized(mut self) -> Self {
        while self.limbs.last().is_some_and(|limb| *limb == 0) {
            self.limbs.pop();
        }
        self
    }
}

impl PartialEq for Big {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Big {}

impl PartialOrd for Big {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Big {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.limbs.len().cmp(&other.limbs.len()) {
            Ordering::Equal => self
                .limbs
                .iter()
                .rev()
                .zip(other.limbs.iter().rev())
                .find_map(|(left, right)| match left.cmp(right) {
                    Ordering::Equal => None,
                    order => Some(order),
                })
                .unwrap_or(Ordering::Equal),
            order => order,
        }
    }
}

/// `limbs << shift` for `shift < 64`, with one extra limb when `grow` is set.
fn shl_limbs(limbs: &[u64], shift: u32, grow: bool) -> Vec<u64> {
    let mut out = Vec::with_capacity(limbs.len() + 1);
    let mut carry = 0_u64;
    for limb in limbs {
        if shift == 0 {
            out.push(*limb);
        } else {
            out.push((limb << shift) | carry);
            carry = limb >> (64 - shift);
        }
    }
    if grow {
        out.push(carry);
    }
    out
}

/// `limbs >> shift` for `shift < 64`.
fn shr_limbs(limbs: &[u64], shift: u32) -> Big {
    if shift == 0 {
        return Big {
            limbs: limbs.to_vec(),
        };
    }
    let mut out = Vec::with_capacity(limbs.len());
    for (index, limb) in limbs.iter().enumerate() {
        let high = limbs.get(index + 1).map_or(0, |next| next << (64 - shift));
        out.push((limb >> shift) | high);
    }
    Big { limbs: out }
}

fn gcd(mut left: Big, mut right: Big) -> Big {
    // Both sides almost always fit in 128 bits: stay native, no allocation.
    if let (Some(mut a), Some(mut b)) = (left.to_u128(), right.to_u128()) {
        while b != 0 {
            (a, b) = (b, a % b);
        }
        return Big::from_u128(a);
    }
    while !right.is_zero() {
        let remainder = left.divmod(&right).1;
        left = right;
        right = remainder;
    }
    left
}

/// Decimal text with no exponent. Trailing zeros that `normalize` drops stay
/// dropped. `0` stays `0`.
#[must_use]
pub fn format_dec(value: Decimal) -> String {
    let text = value.normalize().to_string();
    if text.contains(['e', 'E']) {
        return value.round_dp(12).normalize().to_string();
    }
    text
}

/// Like [`format_dec`], with at least two digits after the decimal point.
#[must_use]
pub fn format_usd(value: Decimal) -> String {
    let text = format_dec(value);
    let Some((whole, frac)) = text.split_once('.') else {
        return format!("{text}.00");
    };
    if frac.len() >= 2 {
        text
    } else {
        format!("{whole}.{frac}{}", "0".repeat(2 - frac.len()))
    }
}

pub(crate) fn parse_money(raw: &str) -> Result<Decimal, ()> {
    let text = raw.trim_matches(|c| matches!(c, ' ' | '\t' | '\r' | '\n'));
    if text.is_empty() || text.len() > 40 {
        return Err(());
    }
    let bytes = text.as_bytes();
    let mut dot = false;
    for (i, byte) in bytes.iter().enumerate() {
        match byte {
            b'0'..=b'9' => {}
            b'.' if !dot && i > 0 && i + 1 < bytes.len() => dot = true,
            _ => return Err(()),
        }
    }
    Decimal::from_str_exact(text).map_err(|_| ())
}

pub(crate) fn valid_date(text: &str) -> bool {
    let bytes = text.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    let digits_only = bytes
        .iter()
        .enumerate()
        .all(|(i, b)| i == 4 || i == 7 || b.is_ascii_digit());
    if !digits_only {
        return false;
    }
    let Ok(year) = text[0..4].parse::<i32>() else {
        return false;
    };
    let Ok(month) = text[5..7].parse::<u32>() else {
        return false;
    };
    let Ok(day) = text[8..10].parse::<u32>() else {
        return false;
    };
    if !(1..=12).contains(&month) {
        return false;
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let max = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    (1..=max).contains(&day)
}

/// Days since 1970-01-01 of a valid `YYYY-MM-DD` date (proleptic Gregorian).
pub(crate) fn days_from_date(text: &str) -> Option<i64> {
    if !valid_date(text) {
        return None;
    }
    let year: i64 = text[0..4].parse().ok()?;
    let month: i64 = text[5..7].parse().ok()?;
    let day: i64 = text[8..10].parse().ok()?;
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year.rem_euclid(400);
    let shifted = (month + 9) % 12;
    let day_of_year = (153 * shifted + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    Some(era * 146_097 + day_of_era - 719_468)
}

/// Today in UTC as `YYYY-MM-DD`, from the system clock.
pub(crate) fn today_utc() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let days = i64::try_from(secs / 86_400).unwrap_or(0) + 719_468;
    let era = days.div_euclid(146_097);
    let day_of_era = days.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted + 2) / 5 + 1;
    let month = if shifted < 10 {
        shifted + 3
    } else {
        shifted - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

pub(crate) fn money_error(message: impl Into<String>) -> Error {
    Error::new(message)
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;

    use std::cmp::Ordering;

    use super::{format_dec, format_usd, gb_seconds, parse_money, valid_date, Big};

    #[test]
    fn one_gb_second_at_scale_28_is_exact() {
        use super::{exact_decimal, exact_duration};
        let rate: Decimal = "0.0000000000000000000000000001".parse().unwrap();
        assert_eq!(rate.scale(), 28, "scale {}", rate.scale());
        assert_eq!(rate.mantissa(), 1);
        let got = exact_decimal(&exact_duration(1024, 1000, rate)).unwrap();
        assert_eq!(format_dec(got), "0.0000000000000000000000000001");
    }

    #[test]
    fn days_from_date_counts_calendar_days() {
        use super::days_from_date;
        assert_eq!(days_from_date("1970-01-01"), Some(0));
        assert_eq!(days_from_date("2000-03-01"), Some(11017));
        assert_eq!(days_from_date("2026-11-09"), Some(20766));
        let leap = days_from_date("2028-03-01").unwrap() - days_from_date("2028-02-28").unwrap();
        assert_eq!(leap, 2);
        assert_eq!(days_from_date("2025-02-29"), None);
    }

    #[test]
    fn today_utc_is_a_valid_date_after_this_code_was_written() {
        let today = super::today_utc();
        assert!(valid_date(&today), "{today}");
        assert!(today.as_str() >= "2026-11-20", "{today}");
    }

    #[test]
    fn one_gb_is_1024_mb() {
        assert_eq!(gb_seconds(1024, 1000).unwrap(), Decimal::ONE);
        assert_ne!(gb_seconds(1000, 1000).unwrap(), Decimal::ONE);
        assert_eq!(format_dec(gb_seconds(1024, 1000).unwrap()), "1");
    }

    #[test]
    fn gb_seconds_at_u64_max_does_not_panic() {
        // 10240 * u64::MAX is about 1.9e23, which fits in Decimal, so the
        // result is exact: u64::MAX / 100.
        let got = gb_seconds(10240, u64::MAX).unwrap();
        assert_eq!(got.to_string(), "184467440737095516.15");
        assert_eq!(got, "184467440737095516.15".parse::<Decimal>().unwrap());
    }

    #[test]
    fn one_millisecond_at_128_mb() {
        let gb = gb_seconds(128, 1).unwrap();
        assert_eq!(gb, Decimal::from(128) / Decimal::from(1_024_000));
        assert_eq!(format_dec(gb), "0.000125");
    }

    #[test]
    fn page_example_is_540000_gb_seconds() {
        // 3_000_000 requests * 120 ms = 360_000 seconds at 1536 MB.
        let gb = gb_seconds(1536, 360_000_000).unwrap();
        assert_eq!(gb, Decimal::from(540_000));
        let usd = gb * "0.0000166667".parse::<Decimal>().unwrap();
        assert_eq!(format_usd(usd), "9.000018");
    }

    #[test]
    fn money_rejects_exponent_sign_and_empty() {
        assert!("0.20".parse::<Decimal>().is_ok());
        assert_eq!(
            parse_money("0.20").unwrap(),
            "0.2".parse::<Decimal>().unwrap()
        );
        assert!(parse_money("0.00000000000000000000000000001").is_err());
        assert!(parse_money("99999999999999999999.99999999999").is_err());
        assert!(parse_money("0.0000000000000000000000000001").is_ok());
        assert!(parse_money("1e-7").is_err());
        assert!(parse_money("1E-7").is_err());
        assert!(parse_money("-1").is_err());
        assert!(parse_money("+1").is_err());
        assert!(parse_money(".2").is_err());
        assert!(parse_money("1.").is_err());
        assert!(parse_money("").is_err());
        assert!(parse_money("0").is_ok());
        assert!(parse_money("0.0").is_ok());
    }

    #[test]
    fn money_trims_only_ascii_whitespace() {
        assert_eq!(
            parse_money(" \t0.20\r\n").unwrap(),
            "0.2".parse::<Decimal>().unwrap()
        );
        for ch in ['\u{a0}', '\u{3000}', '\u{2028}'] {
            assert!(
                parse_money(&format!("{ch}0.20")).is_err(),
                "U+{:04X}",
                ch as u32
            );
            assert!(
                parse_money(&format!("0.20{ch}")).is_err(),
                "U+{:04X}",
                ch as u32
            );
        }
    }

    #[test]
    fn usd_keeps_two_digits_and_exact_fractions() {
        assert_eq!(format_usd(Decimal::ZERO), "0.00");
        assert_eq!(format_usd(Decimal::from(2)), "2.00");
        assert_eq!(format_usd("0.0000004".parse().unwrap()), "0.0000004");
        assert_eq!(format_usd("0.60".parse().unwrap()), "0.60");
    }

    #[test]
    fn dates_follow_the_calendar() {
        assert!(valid_date("2026-11-09"));
        assert!(valid_date("2024-02-29"));
        assert!(!valid_date("2025-02-29"));
        assert!(!valid_date("2026-13-01"));
        assert!(!valid_date("2026-00-01"));
        assert!(!valid_date("2026-04-31"));
        assert!(!valid_date("2026-11-9"));
        assert!(!valid_date("20261109"));
        assert!(valid_date("2000-02-29"));
        assert!(!valid_date("1900-02-29"));
    }

    #[test]
    fn dates_allow_only_digits() {
        assert!(!valid_date("2026-+1-09"));
        assert!(!valid_date("2026-01-+9"));
        assert!(!valid_date("+026-01-09"));
        assert!(!valid_date("2026-0a-09"));
    }
    /// Vectors from Python integers (`divmod`): odd limb counts, a divisor
    /// whose top limb is full, and quotient digits that need the add-back.
    #[test]
    fn divmod_matches_python_integers() {
        // numerator, divisor, quotient, remainder (little-endian limbs)
        type Case<'a> = (&'a [u64], &'a [u64], &'a [u64], &'a [u64]);
        let cases: &[Case] = &[
            (
                &[0x95bd447e7184d454],
                &[0xe2f746b484e61ceb],
                &[],
                &[0x95bd447e7184d454],
            ),
            (
                &[0xb80469b84b2d451c, 0x1],
                &[0xa399d29db61e6ba5],
                &[0x2],
                &[0x70d0c47cdef06dd2],
            ),
            (
                &[0x1b2ba06b6ae12c97, 0xf5b19f71ec7ac8ad],
                &[0xa248cd706ca4a1a9],
                &[0x83939921e7215f8d, 0x1],
                &[0x126d5a4d266d6b82],
            ),
            (
                &[
                    0x2052e52bdc36fc90,
                    0x37b003074f0f4c3c,
                    0x778e6176e500bddf,
                    0xde,
                ],
                &[0x54a5054cda2ec003, 0x1],
                &[0xa9d5d65431b3f256, 0x3006bffd316a4bb4, 0xa7],
                &[0xde0872ad89e6a58e],
            ),
            (
                &[
                    0xa84567f9953a5154,
                    0x4c83cf3b8c0a70a7,
                    0x4c8edfa3169a73bf,
                    0x1dc02e270db317d9,
                    0xd88b71ed78a,
                ],
                &[0xe844c762939ae232, 0x66c164e7c85990d2, 0x3],
                &[0xf91fc8356e7e9142, 0xffc4b92d9606e426, 0x3fa9e31752b],
                &[0xbdd4132dfc91ae70, 0xdaa3243ff62dd948, 0x1],
            ),
            (
                &[
                    0x60ec6db25097c040,
                    0x5df6f817b0992bcf,
                    0x3f84c485e5c4176,
                    0x8c3f846cc6a942d3,
                    0x3da9d8c096d38ff5,
                    0x3af9f30c5a661477,
                    0x7ec0c9226ed91f2e,
                    0xbaa2919c0ded97bb,
                ],
                &[
                    0x5e05c9906be994f2,
                    0x9ce4aa5f0b4702d2,
                    0x851bec1ede27387c,
                    0x50d00787862a73db,
                    0x1,
                ],
                &[
                    0x3ddbe10470357508,
                    0xde6c64dd20153367,
                    0x7bc242a34a0a6642,
                    0x8ddae441ea2cfe09,
                ],
                &[
                    0xd0ed918f7a1e7eb0,
                    0x4ea505d47163bf19,
                    0xe05c802be42b7133,
                    0x4762e8e8bf56f6f1,
                ],
            ),
            (
                &[
                    0x7582329f7ac526d3,
                    0xfc80fa6f7a8d9ccd,
                    0x45d28c06620602e0,
                    0xa386c4a7670c5891,
                    0xe287d75eda7e46,
                    0x936b7ac2bce1cb34,
                    0xf910d68abaeebb48,
                    0x9ff7f0c7592b89f,
                    0x4eaf6d618434d99c,
                    0xf0ae1284a9cc2ff7,
                    0xb418a2c1b2410a6,
                ],
                &[0xaba33d67944782d7],
                &[
                    0x9d91eb1d776cc965,
                    0x352e188edb8dc0c3,
                    0x72dc8323920dc758,
                    0x51f7e8b2bbc96dcf,
                    0x1bbeff87df19981a,
                    0x120c7a96504e790f,
                    0x87d4e231ecb1b0df,
                    0x681e87f9cea20979,
                    0xa52e4250824a3c3e,
                    0x10c9db7feeb1ff42,
                ],
                &[0x1d48a9fdb11fb900],
            ),
            (
                &[
                    0xd2ab5d67c074a644,
                    0x27801375101379c7,
                    0xc9f28a8adecf9580,
                    0xc865d3ee543bcfbb,
                ],
                &[
                    0x56364be9b85993a7,
                    0x214119cf6d84cc79,
                    0xbdcb5d3060bed79,
                    0x63fed0b9fe2fbd72,
                ],
                &[0x2],
                &[
                    0x263ec5944fc17ef6,
                    0xe4fddfd63509e0d5,
                    0xb2391ee4d2b7ba8d,
                    0x68327a57dc54d7,
                ],
            ),
            (
                &[
                    0x4233eb557ff959b7,
                    0x46b1f9d204948682,
                    0x12fcce59b1e9f4fd,
                    0xb938eca42ccd5f71,
                ],
                &[
                    0xec32db3b037b1bf3,
                    0xf06e36b1404cd3a,
                    0x46635a99e910fc8d,
                    0xbdde8fcb603ae5da,
                ],
                &[],
                &[
                    0x4233eb557ff959b7,
                    0x46b1f9d204948682,
                    0x12fcce59b1e9f4fd,
                    0xb938eca42ccd5f71,
                ],
            ),
            (
                &[0xfb68eb68bda03923, 0x9f6d26b2d4ae6af5, 0x1],
                &[0x5b3916640d9792c1, 0x8d5feb548d301caf],
                &[0x2],
                &[0x44f6bea0a27113a1, 0x84ad5009ba4e3197],
            ),
            (
                &[
                    0xe8948dc322aab6fe,
                    0xedeed7391ee6db62,
                    0x5d0991e00822e8e7,
                    0x37c3de4c02228d48,
                    0x8b21ea852a848829,
                    0x7ac0cddb0346ce06,
                    0xc199c0a98a624ac1,
                    0x627d239fdf8adf39,
                    0x4ca8c324a8eb52f1,
                    0x88b6350eedd03c87,
                    0xd9fb18e8eef6aac4,
                    0x6895bc4380f485ea,
                    0xf7572dd88f18de53,
                    0x53623b53b93e295f,
                    0x9581467552608ba9,
                    0x8976886726,
                ],
                &[
                    0xdde5a8811aafd366,
                    0x19b343bb8a01f57a,
                    0x1007e910f9a8eab8,
                    0x6518182b5e200159,
                    0xe4d3921a724a67f6,
                    0x17c4,
                ],
                &[
                    0x478d5c74c984037,
                    0x6eb6a073513ee45f,
                    0x3f50b336b00bc2f6,
                    0xb67111e19cbc6756,
                    0x9a7377b8d6a03548,
                    0x2e6a1fdad5a7ba30,
                    0xc9c55439baaeab12,
                    0xc616b4471ba807cd,
                    0x1e77314e06dde463,
                    0xaad114e58db7b8b5,
                    0x5c883d4,
                ],
                &[
                    0xb827d5e8a57acc14,
                    0x42b2e700346583fe,
                    0x8795fb5a76db4a55,
                    0xc6866f536e77a6fd,
                    0xa27ab52cbaf5115d,
                    0x14e9,
                ],
            ),
            (
                &[0x54e66454a9225a7d, 0xc6e5ff4d2936be1a, 0xd75762ef0e5c6000],
                &[0xe15b72c29e6a1947, 0xbf76e314c4806bd0, 0xfe2d130254a8dc38],
                &[],
                &[0x54e66454a9225a7d, 0xc6e5ff4d2936be1a, 0xd75762ef0e5c6000],
            ),
            (
                &[
                    0x6b9156e7a45c8f2d,
                    0xe5e5b24049469a5e,
                    0xe4ec9f0dfe030f59,
                    0x1,
                ],
                &[0x8eeb4467a5c46004, 0x479dd02e5974cf6b, 0x1],
                &[0x7aebabbf78c8fb55, 0x1],
                &[0x5f62238d2ce4c1d9, 0x7c64dfaf8f6f090],
            ),
            (
                &[
                    0xffffffffffffffff,
                    0xffffffffffffffff,
                    0xffffffffffffffff,
                    0xffffffffffffffff,
                ],
                &[0xffffffffffffffff, 0xffffffffffffffff],
                &[0x1, 0x0, 0x1],
                &[],
            ),
            (
                &[0x0, 0x0, 0x0, 0x1],
                &[0x1, 0x1],
                &[0x0, 0xffffffffffffffff],
                &[0x0, 0x1],
            ),
            (
                &[u64::MAX, u64::MAX, u64::MAX, u64::MAX],
                &[u64::MAX, u64::MAX],
                &[1, 0, 1],
                &[],
            ),
            (
                &[0, 0, 0x8000_0000_0000_0000],
                &[1, 0x8000_0000_0000_0000],
                &[0xffff_ffff_ffff_ffff],
                &[1, 0x7fff_ffff_ffff_ffff],
            ),
        ];
        for (num, den, quot, rem) in cases {
            let (q, r) = Big {
                limbs: num.to_vec(),
            }
            .divmod(&Big {
                limbs: den.to_vec(),
            });
            assert_eq!(q.limbs, *quot, "{num:x?} / {den:x?}");
            assert_eq!(r.limbs, *rem, "{num:x?} % {den:x?}");
        }
    }

    /// `q * d + r == n` and `r < d` over many pseudo-random limb patterns,
    /// including all-ones and all-zero limbs that trigger the add-back.
    #[test]
    fn divmod_identity_holds_on_edge_limbs() {
        let mut state = 0x9e37_79b9_7f4a_7c15_u64;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            match state % 5 {
                0 => 0,
                1 => u64::MAX,
                2 => 1,
                3 => 1 << 63,
                _ => state,
            }
        };
        for round in 0..4000 {
            let num_len = 1 + round % 7;
            let den_len = 1 + (round / 7) % 4;
            let num = Big {
                limbs: (0..num_len).map(|_| next()).collect(),
            }
            .normalized();
            let den = Big {
                limbs: (0..den_len).map(|_| next()).collect(),
            }
            .normalized();
            if den.is_zero() {
                continue;
            }
            let (q, r) = num.divmod(&den);
            assert!(
                r.cmp(&den) == Ordering::Less,
                "{:x?} % {:x?}",
                num.limbs,
                den.limbs
            );
            assert!(
                q.mul(&den).add(&r).cmp(&num) == Ordering::Equal,
                "{:x?} / {:x?}",
                num.limbs,
                den.limbs
            );
            assert!(q.limbs.last().is_none_or(|limb| *limb != 0));
        }
    }
}
