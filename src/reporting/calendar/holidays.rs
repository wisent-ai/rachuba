//! Federal holiday observance and the distinct IRS legal-holiday calendar.

use super::{add_days, last_day_of_month};
use chrono::{Datelike, NaiveDate, Weekday};

// The months that carry a floating holiday, and the ordinal of the weekday it falls on.
const JANUARY: u32 = 1;
const FEBRUARY: u32 = 2;
const MAY: u32 = 5;
const SEPTEMBER: u32 = 9;
const OCTOBER: u32 = 10;
const NOVEMBER: u32 = 11;
const FIRST: u32 = 1;
const SECOND: u32 = 2;
const THIRD: u32 = 3;
const FOURTH: u32 = 4;
/// D.C. Official Code section 1-612.02(a)(11): Emancipation Day begins in 2007.
const DC_EMANCIPATION_DAY_FIRST_YEAR: i32 = 2007;
/// 5 U.S.C. 6103(c): Inauguration Day is January 20 of each fourth year after 1965.
const INAUGURATION_BASE_YEAR: i32 = 1965;
const YEARS_BETWEEN_INAUGURATIONS: i32 = 4;

/// The eleven legal public holidays of 5 U.S.C. 6103(a) designated for `year`,
/// each moved to the day it is actually observed, sorted ascending.
///
/// The floating holidays are computed, not tabulated: the third Monday in
/// January (Birthday of Martin Luther King, Jr.), the third Monday in February
/// (Washington's Birthday), the last Monday in May (Memorial Day), the first
/// Monday in September (Labor Day), the second Monday in October (Columbus
/// Day), and the fourth Thursday in November (Thanksgiving Day). The fixed
/// holidays are January 1, June 19 (Juneteenth National Independence Day),
/// July 4, November 11, and December 25.
///
/// Observation, per 5 U.S.C. 6103(b)(1)(A) and section 3(a) of Executive Order
/// 11582: a holiday falling on a Saturday is observed the preceding Friday, and
/// one falling on a Sunday the following Monday.
///
/// The returned dates are the holidays *designated for* `year`, so the New
/// Year's Day observance can land on December 31 of `year - 1`. That is not an
/// off-by-one: January 1, 2028 is a Saturday, and OPM's published 2028 schedule
/// lists "Friday, December 31" as the New Year's Day holiday. The same happened
/// for 2022, observed Friday, December 31, 2021.
///
/// Inauguration Day is deliberately absent. It is a legal public holiday under
/// 5 U.S.C. 6103(c), not 6103(a), it applies only to employees in the
/// Washington area, and it is not one of the eleven. It does affect IRS
/// due dates, so `is_irs_legal_holiday` accounts for it separately.
pub fn federal_holidays(year: i32) -> Vec<NaiveDate> {
    let fixed = |month, day| {
        NaiveDate::from_ymd_opt(year, month, day).expect("fixed holiday is a real date")
    };
    let mut days = vec![
        observed(fixed(1, 1)),                                     // New Year's Day
        observed(nth_weekday_of_month(year, 1, Weekday::Mon, 3)),  // MLK's Birthday
        observed(nth_weekday_of_month(year, 2, Weekday::Mon, 3)),  // Washington's Birthday
        observed(last_weekday_of_month(year, 5, Weekday::Mon)),    // Memorial Day
        observed(fixed(6, 19)),                                    // Juneteenth
        observed(fixed(7, 4)),                                     // Independence Day
        observed(nth_weekday_of_month(year, 9, Weekday::Mon, 1)),  // Labor Day
        observed(nth_weekday_of_month(year, 10, Weekday::Mon, 2)), // Columbus Day
        observed(fixed(11, 11)),                                   // Veterans Day
        observed(nth_weekday_of_month(year, 11, Weekday::Thu, 4)), // Thanksgiving Day
        observed(fixed(12, 25)),                                   // Christmas Day
    ];
    days.sort_unstable();
    days
}

pub(super) fn is_weekend(d: NaiveDate) -> bool {
    matches!(d.weekday(), Weekday::Sat | Weekday::Sun)
}

/// The `n`th `weekday` of a month, `n` counting from 1.
fn nth_weekday_of_month(year: i32, month: u32, weekday: Weekday, n: u32) -> NaiveDate {
    let first = NaiveDate::from_ymd_opt(year, month, 1).expect("the 1st of a real month");
    let offset = (7 + weekday.num_days_from_monday() - first.weekday().num_days_from_monday()) % 7;
    NaiveDate::from_ymd_opt(year, month, 1 + offset + 7 * (n - 1))
        .expect("caller asks only for an nth weekday that exists")
}

/// The last `weekday` of a month, for Memorial Day's "last Monday in May".
fn last_weekday_of_month(year: i32, month: u32, weekday: Weekday) -> NaiveDate {
    let last = last_day_of_month(year, month);
    let back = (7 + last.weekday().num_days_from_monday() - weekday.num_days_from_monday()) % 7;
    add_days(last, -(back as i32))
}

/// The day a holiday falling on `d` is actually observed: the preceding Friday
/// for a Saturday, the following Monday for a Sunday, otherwise `d` itself.
///
/// 5 U.S.C. 6103(b)(1)(A) for the Saturday case, section 3(a) of Executive
/// Order 11582 for the Sunday case, both as summarized by OPM: "when a holiday
/// falls on a nonworkday -- Saturday or Sunday -- the holiday usually is
/// observed on Monday (if the holiday falls on Sunday) or Friday (if the
/// holiday falls on Saturday)."
fn observed(d: NaiveDate) -> NaiveDate {
    match d.weekday() {
        Weekday::Sat => add_days(d, -1),
        Weekday::Sun => add_days(d, 1),
        _ => d,
    }
}

/// True when `d` is one of the eleven dates designated by 5 U.S.C. 6103(a),
/// before any observation shift.
///
/// Answers for a single date without building the year's list, so the business
/// day scan stays allocation free.
fn is_designated_federal_holiday(d: NaiveDate) -> bool {
    let year = d.year();
    match (d.month(), d.day()) {
        (1, 1) => true,   // New Year's Day
        (6, 19) => true,  // Juneteenth National Independence Day
        (7, 4) => true,   // Independence Day
        (11, 11) => true, // Veterans Day
        (12, 25) => true, // Christmas Day
        (JANUARY, _) => d == nth_weekday_of_month(year, JANUARY, Weekday::Mon, THIRD),
        (FEBRUARY, _) => d == nth_weekday_of_month(year, FEBRUARY, Weekday::Mon, THIRD),
        (MAY, _) => d == last_weekday_of_month(year, MAY, Weekday::Mon),
        (SEPTEMBER, _) => d == nth_weekday_of_month(year, SEPTEMBER, Weekday::Mon, FIRST),
        (OCTOBER, _) => d == nth_weekday_of_month(year, OCTOBER, Weekday::Mon, SECOND),
        (NOVEMBER, _) => d == nth_weekday_of_month(year, NOVEMBER, Weekday::Thu, FOURTH),
        _ => false,
    }
}

/// True when a federal holiday is observed on `d`.
///
/// A holiday is observed on `d` when it is designated for `d`, or when `d` is
/// the Friday before a Saturday holiday, or the Monday after a Sunday one. Only
/// the fixed-date holidays can fall on a weekend; the floating ones are already
/// Mondays and a Thursday. The Friday branch is also what makes December 31 a
/// holiday when the coming January 1 is a Saturday, with no year bookkeeping.
pub(super) fn is_observed_federal_holiday(d: NaiveDate) -> bool {
    match d.weekday() {
        Weekday::Sat | Weekday::Sun => false,
        Weekday::Fri => {
            is_designated_federal_holiday(d) || is_designated_federal_holiday(add_days(d, 1))
        }
        Weekday::Mon => {
            is_designated_federal_holiday(d) || is_designated_federal_holiday(add_days(d, -1))
        }
        _ => is_designated_federal_holiday(d),
    }
}

/// The observed date of District of Columbia Emancipation Day in `year`, or
/// `None` before the holiday existed.
///
/// D.C. Official Code section 1-612.02(a)(11): "Beginning in the year 2007,
/// District of Columbia Emancipation Day, April 16th of each year." The
/// observation rule of section 1-612.02(b)(1) matches the federal one, Saturday
/// to the preceding Friday and Sunday to the following Monday.
pub(super) fn dc_emancipation_day(year: i32) -> Option<NaiveDate> {
    if year < DC_EMANCIPATION_DAY_FIRST_YEAR {
        return None;
    }
    Some(observed(
        NaiveDate::from_ymd_opt(year, 4, 16).expect("April has a 16th"),
    ))
}

/// The observed date of Inauguration Day in `year`, or `None` in the three
/// years out of four that do not have one.
///
/// 5 U.S.C. 6103(c): "January 20 of each fourth year after 1965, Inauguration
/// Day, is a legal public holiday", and "when January 20 of any fourth year
/// after 1965 falls on Sunday, the next succeeding day selected for the public
/// observance of the inauguration of the President is a legal public holiday."
///
/// A Saturday January 20 gets no in-lieu-of day at all. OPM's note on the 2029
/// schedule is explicit: "Inauguration Day, January 20, 2029, falls on a
/// Saturday... In this instance, Inauguration Day is not observed on another
/// day. There is no in-lieu-of holiday for employees who are not regularly
/// scheduled to work on Inauguration Day (5 U.S.C. 6103(c))." That is the one
/// place the observation rules of [`observed`] do not apply.
pub(super) fn inauguration_day(year: i32) -> Option<NaiveDate> {
    if year <= INAUGURATION_BASE_YEAR || (year - INAUGURATION_BASE_YEAR) % YEARS_BETWEEN_INAUGURATIONS != 0 {
        return None;
    }
    let designated = NaiveDate::from_ymd_opt(year, 1, 20).expect("January has a 20th");
    match designated.weekday() {
        Weekday::Sat => None,
        Weekday::Sun => Some(add_days(designated, 1)),
        _ => Some(designated),
    }
}

/// True when `d` is a "legal holiday" in the sense 26 U.S.C. 7503 gives the
/// term: a legal holiday in the District of Columbia.
///
/// That is the observed federal holidays plus the two the District observes on
/// its own account. IRS Publication 15 (2021), section 11, lists exactly this
/// set for an inauguration year: "January 18 - Birthday of Martin Luther King,
/// Jr.; January 20 - Inauguration Day; February 15 - Washington's Birthday;
/// April 16 - District of Columbia Emancipation Day; ..." and Publication 15
/// (2026) lists "April 16 - District of Columbia Emancipation Day" among the
/// 2026 legal holidays.
///
/// Statewide legal holidays are excluded. 26 CFR 31.6302-1(c)(4): "the term
/// 'legal holiday' does not include other Statewide legal holidays", and
/// Publication 15 adds that a statewide holiday never delays a federal tax
/// deposit. Nothing here is state specific, so nothing needs excluding.
pub(super) fn is_irs_legal_holiday(d: NaiveDate) -> bool {
    is_observed_federal_holiday(d)
        || dc_emancipation_day(d.year()) == Some(d)
        || inauguration_day(d.year()) == Some(d)
}

/// True when a deposit or a return can be made on `d`, under 26 CFR
/// 31.6302-1(c)(4): "Business days include every calendar day other than
/// Saturdays, Sundays, or legal holidays", with "legal holiday" in the section
/// 7503 sense.
pub(super) fn is_irs_business_day(d: NaiveDate) -> bool {
    !is_weekend(d) && !is_irs_legal_holiday(d)
}

/// `d` itself when it is an IRS business day, otherwise the next one.
///
/// 26 U.S.C. 7503: "When the last day prescribed... falls on Saturday, Sunday,
/// or a legal holiday, the performance of such act shall be considered timely
/// if it is performed on the next succeeding day which is not a Saturday,
/// Sunday, or a legal holiday."
pub(super) fn irs_business_day_on_or_after(d: NaiveDate) -> NaiveDate {
    let mut candidate = d;
    while !is_irs_business_day(candidate) {
        candidate = add_days(candidate, 1);
    }
    candidate
}
