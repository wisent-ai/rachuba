//! Quarterly and annual federal filing due dates.

use super::holidays::irs_business_day_on_or_after;
use super::{last_day_of_month, quarter_end};
use chrono::{Datelike, NaiveDate};

/// When the Form 941 for `quarter` of `year` is due: the last day of the first
/// calendar month following the quarter, moved to the next business day when
/// that falls on a weekend or legal holiday.
///
/// 26 CFR 31.6071(a)-1(a)(1): the return "shall be filed on or before the last
/// day of the first calendar month following the period for which it is made."
/// So April 30, July 31, October 31, and January 31 of the following year. The
/// weekend and holiday shift is 26 CFR 31.6071(a)-1(d), which routes to
/// section 301.7503-1 and thus to 26 U.S.C. 7503.
///
/// The same sentence of the regulation grants an extension this function does
/// not take: a Form 941 "may be filed on or before the 10th day of the second
/// calendar month following such period if timely deposits under section
/// 6302(c) of the Code and the regulations have been made in full payment of
/// such taxes due for the period." Whether that condition holds depends on the
/// deposit history, not the calendar, so the earlier date is returned.
///
/// # Panics
///
/// Panics when `quarter` is outside 1 through 4, by way of [`quarter_end`].
pub fn form_941_due(year: i32, quarter: u32) -> NaiveDate {
    let period_end = quarter_end(year, quarter);
    let (filing_year, filing_month) = match period_end.month() {
        12 => (year + 1, 1),
        month => (year, month + 1),
    };
    irs_business_day_on_or_after(last_day_of_month(filing_year, filing_month))
}

/// When the Form 940 for `year` is due: January 31 of the following year, moved
/// to the next business day when that falls on a weekend or legal holiday.
///
/// 26 CFR 31.6071(a)-1(c): the FUTA return "shall be filed on or before the
/// last day of the first calendar month following the period for which it is
/// made", and the period is the calendar year. The shift is 26 U.S.C. 7503 by
/// way of 26 CFR 31.6071(a)-1(d).
///
/// The same paragraph continues: "However, a return may be filed on or before
/// the 10th day of the second calendar month following such period if timely
/// deposits under section 6302(c) of the Code and the regulations thereunder
/// have been made in full payment of such taxes due for the period." So when
/// every FUTA deposit for the year was made on time and in full, the due date
/// is February 10 rather than January 31, itself subject to the same weekend
/// and holiday shift. This function returns the January 31 date, because
/// whether the condition is met is a fact about the deposit ledger rather than
/// about the calendar, and January 31 is correct either way.
pub fn form_940_due(year: i32) -> NaiveDate {
    irs_business_day_on_or_after(january_31_following(year))
}

/// When the Form W-2 for `year` is due to the Social Security Administration:
/// January 31 of the following year, moved to the next business day when that
/// falls on a weekend or legal holiday.
///
/// 26 U.S.C. 6071(c): "Forms W-2 and W-3 and any returns or statements required
/// by the Secretary to report nonemployee compensation shall be filed on or
/// before January 31 of the year following the calendar year to which such
/// returns relate." Enacted by the PATH Act, Pub. L. 114-113, division Q, title
/// II, section 201(a), 18 December 2015, effective for returns relating to
/// calendar years after 2015. See also 26 CFR 31.6071(a)-1(a)(3)(i).
///
/// There is no electronic-filing extension for Form W-2. The March 31 date of
/// 26 U.S.C. 6071(b) reaches only returns under subpart B of part III, and
/// section 201(c) of the PATH Act narrowed it further. January 31 is also the
/// date by which the employee's copy must be furnished, under 26 U.S.C. 6051(a).
pub fn form_w2_due(year: i32) -> NaiveDate {
    irs_business_day_on_or_after(january_31_following(year))
}

/// The four required installments of estimated income tax a year has.
pub const ESTIMATED_TAX_INSTALLMENTS: u32 = 4;

/// When installment `number` (1 to 4) of an individual's estimated tax for
/// `year` is due: April 15, June 15 and September 15 of the year and January
/// 15 of the next, each moved to the next business day when it falls on a
/// weekend or legal holiday.
///
/// 26 U.S.C. 6654(c)(2) for a calendar-year taxpayer; the shift is 26 U.S.C.
/// 7503.
///
/// # Panics
///
/// Panics when `number` is outside 1 through 4. It comes from a loop over
/// [`ESTIMATED_TAX_INSTALLMENTS`], never from input.
pub fn estimated_tax_due(year: i32, number: u32) -> NaiveDate {
    let (y, month) = match number {
        1 => (year, 4),
        2 => (year, 6),
        3 => (year, 9),
        4 => (year + 1, 1),
        other => panic!("estimated tax installment {other} does not exist"),
    };
    let statutory = NaiveDate::from_ymd_opt(y, month, 15).expect("the 15th exists in every month");
    irs_business_day_on_or_after(statutory)
}

/// January 31 of the year after `year`, the statutory date shared by Form 940
/// and Form W-2 before any weekend or holiday shift.
fn january_31_following(year: i32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year + 1, 1, 31).expect("January has 31 days")
}
