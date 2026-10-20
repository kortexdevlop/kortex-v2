//! Gregorian calendar arithmetic, bounded to years 0001–9999, without platform access.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Date {
    pub year: i64,
    pub month: u32,
    pub day: u32,
}

#[must_use]
pub const fn leap(year: i64) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

#[must_use]
pub const fn month_days(year: i64, month: u32) -> u32 {
    match month {
        2 => {
            if leap(year) {
                29
            } else {
                28
            }
        }
        4 | 6 | 9 | 11 => 30,
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        _ => 0,
    }
}

const fn before_year(year: i64) -> i64 {
    let y = year - 1;
    365 * y + y / 4 - y / 100 + y / 400
}
const EPOCH: i64 = before_year(1970);

impl Date {
    #[must_use]
    pub fn new(year: i64, month: u32, day: u32) -> Option<Self> {
        if !(1..=9999).contains(&year)
            || !(1..=12).contains(&month)
            || day == 0
            || day > month_days(year, month)
        {
            return None;
        }
        Some(Self { year, month, day })
    }

    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let b = text.as_bytes();
        if b.len() != 10
            || b[4] != b'-'
            || b[7] != b'-'
            || !b
                .iter()
                .enumerate()
                .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
        {
            return None;
        }
        Self::new(
            text[..4].parse().ok()?,
            text[5..7].parse().ok()?,
            text[8..].parse().ok()?,
        )
    }

    /// Signed day offset from 1970-01-01.
    #[must_use]
    pub fn ordinal(self) -> i64 {
        before_year(self.year) - EPOCH
            + (1..self.month)
                .map(|m| i64::from(month_days(self.year, m)))
                .sum::<i64>()
            + i64::from(self.day)
            - 1
    }

    #[must_use]
    pub fn from_ordinal(day: i64) -> Option<Self> {
        let absolute = day.checked_add(EPOCH)?;
        if !(0..before_year(10000)).contains(&absolute) {
            return None;
        }
        let (mut low, mut high) = (1, 10000);
        while low + 1 < high {
            let mid = (low + high) / 2;
            if before_year(mid) <= absolute {
                low = mid;
            } else {
                high = mid;
            }
        }
        let mut left = absolute - before_year(low);
        for month in 1..=12 {
            let days = i64::from(month_days(low, month));
            if left < days {
                return Self::new(low, month, u32::try_from(left + 1).ok()?);
            }
            left -= days;
        }
        None
    }

    #[must_use]
    pub fn add_days(self, days: i64) -> Option<Self> {
        Self::from_ordinal(self.ordinal().checked_add(days)?)
    }

    /// Monday = 0, Sunday = 6.
    #[must_use]
    pub fn weekday(self) -> i64 {
        (self.ordinal() + 3).rem_euclid(7)
    }

    #[must_use]
    pub fn iso(self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

/// Parse an exact UTC timestamp: YYYY-MM-DDTHH:MM:SS[.mmm]Z.
#[must_use]
pub fn parse_utc(text: &str) -> Option<i64> {
    let b = text.as_bytes();
    if !matches!(b.len(), 20 | 24)
        || b[10] != b'T'
        || b[13] != b':'
        || b[16] != b':'
        || *b.last()? != b'Z'
    {
        return None;
    }
    let date = Date::parse(text.get(..10)?)?;
    let hour: i64 = text.get(11..13)?.parse().ok()?;
    let minute: i64 = text.get(14..16)?.parse().ok()?;
    let second: i64 = text.get(17..19)?.parse().ok()?;
    if !(0..24).contains(&hour) || !(0..60).contains(&minute) || !(0..60).contains(&second) {
        return None;
    }
    let millis = if b.len() == 24 {
        if b[19] != b'.' || !b[20..23].iter().all(u8::is_ascii_digit) {
            return None;
        }
        text[20..23].parse::<i64>().ok()?
    } else {
        0
    };
    // Prevent parse() from accepting signs in fixed-width clock fields.
    if !b[11..13]
        .iter()
        .chain(&b[14..16])
        .chain(&b[17..19])
        .all(u8::is_ascii_digit)
    {
        return None;
    }
    Some(date.ordinal() * 86_400_000 + (hour * 3600 + minute * 60 + second) * 1000 + millis)
}

#[must_use]
pub fn format_utc(millis: i64) -> Option<String> {
    let date = Date::from_ordinal(millis.div_euclid(86_400_000))?;
    let time = millis.rem_euclid(86_400_000);
    Some(format!(
        "{}T{:02}:{:02}:{:02}.{:03}Z",
        date.iso(),
        time / 3_600_000,
        time / 60_000 % 60,
        time / 1000 % 60,
        time % 1000
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn calendar_boundaries_and_roundtrips() {
        for text in [
            "0001-01-01",
            "1900-03-01",
            "1970-01-01",
            "2000-02-29",
            "2024-12-31",
            "9999-12-31",
        ] {
            let date = Date::parse(text).expect("date");
            assert_eq!(Date::from_ordinal(date.ordinal()), Some(date));
            assert_eq!(date.iso(), text);
        }
        assert_eq!(Date::parse("1970-01-01").expect("epoch").ordinal(), 0);
        assert_eq!(
            Date::parse("2024-03-01")
                .expect("date")
                .add_days(-1)
                .expect("previous")
                .iso(),
            "2024-02-29"
        );
        for bad in [
            "1900-02-29",
            "2024-13-01",
            "0000-01-01",
            "2024-01-00",
            "2024-1-01",
            "é000-01-01",
        ] {
            assert!(Date::parse(bad).is_none());
        }
    }
    #[test]
    fn utc_negative_epoch_and_invalid_clock() {
        for millis in [
            -62135596800000,
            -1001,
            -1,
            0,
            1,
            1709210096789,
            253402300799999,
        ] {
            assert_eq!(
                parse_utc(&format_utc(millis).expect("format")),
                Some(millis)
            );
        }
        assert!(parse_utc("2024-02-30T12:00:00Z").is_none());
        assert!(parse_utc("2024-01-01T24:00:00Z").is_none());
        assert!(format_utc(i64::MAX).is_none());
    }
}
