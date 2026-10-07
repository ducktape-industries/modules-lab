//! Numbers and times as the screen writes them. Every price, amount and
//! balance is the program's `u64` counting 1e-8 of a unit, so nothing here
//! goes through a float: a number is written and read back exactly.
use ducktape_view_guest::design;
use trade::{MARKETS, UNIT};

/// The decimal places `UNIT` holds.
const PLACES: u32 = 8;

/// The decimals a step of prices or amounts is written with: 0.1 takes one.
pub(crate) fn decimals(step: u64) -> u32 {
    (0..PLACES)
        .find(|places| step.is_multiple_of(10u64.pow(PLACES - places)))
        .unwrap_or(PLACES)
}

/// `value` with `decimals` places, thousands grouped: `63,589.1`. What the
/// places do not hold is cut, not rounded, so a balance never reads as
/// more than it is.
pub(crate) fn fixed(value: u64, decimals: u32) -> String {
    let shown = value / 10u64.pow(PLACES - decimals);
    let one = 10u64.pow(decimals);
    let whole = design::grouped(shown / one);
    match decimals {
        0 => whole,
        _ => format!(
            "{whole}.{:0places$}",
            shown % one,
            places = decimals as usize
        ),
    }
}

/// An amount of a base asset: four places, the program's `LOT`.
pub(crate) fn amount(value: u64) -> String {
    fixed(value, 4)
}

/// A sum of a quote asset: two places.
pub(crate) fn quote(value: u64) -> String {
    fixed(value, 2)
}

/// What an order comes to in a quote asset, to the nearest hundredth: a
/// sum of prices times amounts holds more places than a balance is shown
/// with.
pub(crate) fn total(value: u64) -> String {
    quote(value.saturating_add(UNIT / 200))
}

/// A balance of asset `asset`: what markets are paid in reads as a sum,
/// what they trade as an amount.
pub(crate) fn balance(asset: u8, value: u64) -> String {
    match MARKETS.iter().any(|market| market.quote == asset) {
        true => quote(value),
        false => amount(value),
    }
}

/// A candle's or a day's volume: whole above a hundred, else as an amount.
pub(crate) fn volume(value: u64) -> String {
    match value >= 100 * UNIT {
        true => fixed(value, 0),
        false => amount(value),
    }
}

/// What a field holds as a count of 1e-8, grouping commas allowed: `None`
/// when it is not a number, has more than eight places or does not fit.
pub(crate) fn typed(text: &str) -> Option<u64> {
    let text = text.trim().replace(',', "");
    let (whole, fraction) = text.split_once('.').unwrap_or((&text, ""));
    let digits = |part: &str| part.bytes().all(|byte| byte.is_ascii_digit());
    if whole.is_empty() && fraction.is_empty() || !digits(whole) || !digits(fraction) {
        return None;
    }
    if fraction.len() > PLACES as usize {
        return None;
    }
    let whole: u64 = match whole {
        "" => 0,
        whole => whole.parse().ok()?,
    };
    let fraction: u64 = format!("{fraction:0<places$}", places = PLACES as usize)
        .parse()
        .ok()?;
    whole.checked_mul(UNIT)?.checked_add(fraction)
}

/// What `amount` costs at `price`, in the quote: `price * amount / UNIT`,
/// as the program counts it.
pub(crate) fn cost(price: u64, amount: u64) -> u64 {
    let cost = u128::from(price) * u128::from(amount) / u128::from(UNIT);
    u64::try_from(cost).unwrap_or(u64::MAX)
}

/// How far `last` is from `before`, in hundredths of a percent; `None`
/// when there was no price before.
pub(crate) fn change(last: u64, before: u64) -> Option<i64> {
    let moved = (i128::from(last) - i128::from(before)) * 10_000;
    i64::try_from(moved.checked_div(i128::from(before))?).ok()
}

/// Hundredths of a percent, signed: `+1.00%`.
pub(crate) fn percent(hundredths: i64) -> String {
    let sign = if hundredths < 0 { '-' } else { '+' };
    let size = hundredths.unsigned_abs();
    format!("{sign}{}.{:02}%", size / 100, size % 100)
}

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// A block's time in UTC: the month's place, the day, and the hour, minute
/// and second.
fn utc(millis: u64) -> (usize, u64, u64, u64, u64) {
    let seconds = millis / 1000;
    let (days, of_day) = (seconds / 86_400, seconds % 86_400);
    // days since 1970-01-01 to a civil date (Howard Hinnant's algorithm)
    let doe = (days + 719_468) % 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    (
        month as usize - 1,
        day,
        of_day / 3_600,
        of_day % 3_600 / 60,
        of_day % 60,
    )
}

/// The time of day: `14:02:41`.
pub(crate) fn time(millis: u64) -> String {
    let (_, _, hour, minute, second) = utc(millis);
    format!("{hour:02}:{minute:02}:{second:02}")
}

/// The day: `Oct 6`.
pub(crate) fn day(millis: u64) -> String {
    let (month, day, ..) = utc(millis);
    format!("{} {day}", MONTHS[month])
}

/// The day and the minute: `Oct 6 08:00`.
pub(crate) fn day_time(millis: u64) -> String {
    let (month, day, hour, minute, _) = utc(millis);
    format!("{} {day} {hour:02}:{minute:02}", MONTHS[month])
}

/// A time on the chart's axis: the day where a day starts, the time of day
/// inside one.
pub(crate) fn axis(millis: u64) -> String {
    match utc(millis) {
        (month, day, 0, 0, _) => format!("{} {day}", MONTHS[month]),
        (_, _, hour, minute, _) => format!("{hour:02}:{minute:02}"),
    }
}
