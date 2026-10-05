//! Federal payroll calendar: business days, deposit due dates, and filing
//! due dates.
//!
//! This module owns every date computation the engine performs. The holidays
//! it moves dates around are not computed here: they are the year's
//! [`LegalHolidays`] table, transcribed from the published schedules, and a
//! date that table does not cover is refused rather than read as a working day.
//!
//! Two different definitions of "business day" govern a payroll, and this
//! module deliberately keeps them apart:
//!
//! * The Department of Labor definition, 29 CFR 2510.3-102(e): "the term
//!   business day means any day other than a Saturday, Sunday or any day
//!   designated as a holiday by the Federal Government." That is
//!   [`is_business_day`], and it governs the retirement deferral safe harbor
//!   in [`deferral_remittance_due`].
//! * The Internal Revenue Code definition, 26 U.S.C. 7503: "the term 'legal
//!   holiday' means a legal holiday in the District of Columbia." The District
//!   observes one holiday the Federal Government does not, Emancipation Day on
//!   April 16, and every fourth year it observes Inauguration Day. IRS
//!   Publication 15 lists both among the legal holidays for the deposit rules.
//!   The IRS due-date functions here use that wider set through
//!   [`LegalHolidays::is_irs_legal_holiday`].
//!
//! Collapsing the two sets would schedule a deposit on a day the Treasury is
//! closed, which is a failure-to-deposit penalty under section 6656. See the
//! warning on [`is_business_day`].
//!
//! Sources, each read in full while writing this module:
//!
//!   5 U.S.C. 6103, Holidays. Juneteenth National Independence Day added by
//!     Pub. L. 117-17, section 2, 17 June 2021, 135 Stat. 287.
//!   Executive Order 11582, 11 February 1971, 36 FR 2957, section 3(a)
//!     (holiday falling on Sunday observed the next workday).
//!   OPM, "Federal Holidays", published schedules for 2020 through 2030,
//!     read 28 August 2026.
//!     https://www.opm.gov/policy-data-oversight/pay-leave/federal-holidays/
//!   26 CFR 31.6302-1, Deposit rules for taxes under the Federal Insurance
//!     Contributions Act (FICA) and withheld income taxes.
//!   26 CFR 31.6071(a)-1, Time for filing returns and other documents,
//!     as applicable to returns filed on or after 30 January 2020.
//!   26 U.S.C. 6071(c) (Forms W-2 and W-3) and 26 U.S.C. 7503.
//!   29 CFR 2510.3-102, Definition of "plan assets" - participant
//!     contributions, as amended at 75 FR 2076, 14 January 2010.
//!   D.C. Official Code section 1-612.02, Legal public holidays.
//!   IRS Publication 15 (2026), Employer's Tax Guide, section 11 "Depositing
//!     Taxes", subsections "Monthly Deposit Schedule", "Semiweekly Deposit
//!     Schedule", "Deposits Due on Business Days Only", and "Legal holiday".
//!     https://www.irs.gov/publications/p15
//!   IRS Publication 15 (2021), section 11, "Legal holiday" list, which is the
//!     authority for treating Inauguration Day as a legal holiday.
//!     https://www.irs.gov/pub/irs-prior/p15--2021.pdf

/// The calendar year of a payroll is four quarters of three months.
pub const QUARTERS_PER_YEAR: u32 = 4;
pub const MONTHS_PER_QUARTER: u32 = 3;

mod deposits;
mod filings;
mod holidays;

use anyhow::Result;
use chrono::{Datelike, NaiveDate};
pub use deposits::employment_tax_deposit_due;
pub use filings::{estimated_tax_due, form_940_due, form_941_due, form_w2_due};
use holidays::is_weekend;
pub use holidays::{Holiday, LegalHolidays};

/// Which set of deposit rules applies to an employer for a calendar year.
///
/// The choice is an annual determination based on the employment taxes reported
/// in the lookback period: $50,000 or less makes an employer a monthly
/// depositor, more than $50,000 a semiweekly depositor. 26 CFR 31.6302-1(b)(2)
/// and (b)(3). This module does not make that determination; it only applies
/// the resulting rule in [`employment_tax_deposit_due`].
///
/// The $100,000 next-day rule of 26 CFR 31.6302-1(c)(3) overrides both
/// schedules and is not modeled here. A single-employee C-corp cannot reach a
/// $100,000 single-day liability without wages far above any plausible salary,
/// and modeling it would require the liability amount rather than only dates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DepositSchedule {
    /// Deposit a calendar month's taxes by the 15th of the following month.
    Monthly,
    /// Deposit by the Wednesday or Friday following the semiweekly period.
    Semiweekly,
}

/// True when `d` is a business day under the Department of Labor definition:
/// not a Saturday, not a Sunday, and not an observed federal holiday.
///
/// This is verbatim 29 CFR 2510.3-102(e), "any day other than a Saturday,
/// Sunday or any day designated as a holiday by the Federal Government", and it
/// is the predicate behind [`next_business_day`], [`add_business_days`], and
/// [`deferral_remittance_due`].
///
/// Do not use this for IRS due dates. 26 U.S.C. 7503 defines "legal holiday" as
/// a legal holiday in the District of Columbia, which is a strictly larger set:
/// it adds District of Columbia Emancipation Day on April 16 and, every fourth
/// year, Inauguration Day. IRS Publication 15 (2026) lists April 16 among the
/// 2026 legal holidays, and Publication 15 (2021) lists both April 16 and
/// January 20. The IRS due-date functions in this module already apply the
/// wider set through [`LegalHolidays::is_irs_legal_holiday`]; routing them through
/// this function instead would move a deposit due date onto a day the District
/// is closed. The two predicates are separate on purpose. Do not merge them.
pub fn is_business_day(holidays: &LegalHolidays, d: NaiveDate) -> Result<bool> {
    let holiday = holidays.is_federal_holiday(d)?;
    Ok(!is_weekend(d) && !holiday)
}

/// The first business day strictly after `d`, under the DOL predicate
/// [`is_business_day`].
///
/// Strictly after: passing a business day returns the following one. To move a
/// due date that may already be valid, test [`is_business_day`] first.
pub fn next_business_day(holidays: &LegalHolidays, d: NaiveDate) -> Result<NaiveDate> {
    let mut next = add_days(d, 1);
    while !is_business_day(holidays, next)? {
        next = add_days(next, 1);
    }
    Ok(next)
}

/// The `n`th business day following `d`, counting `d` itself as day zero
/// whether or not it is a business day.
///
/// This is the counting convention of 29 CFR 2510.3-102(a)(2)(i), which runs
/// from "the day on which such amount would otherwise have been payable to the
/// participant in cash" and asks for the 7th business day following it.
/// `n == 0` returns `d` unchanged.
pub fn add_business_days(holidays: &LegalHolidays, d: NaiveDate, n: u32) -> Result<NaiveDate> {
    let mut current = d;
    for _ in 0..n {
        current = next_business_day(holidays, current)?;
    }
    Ok(current)
}

/// The calendar quarter containing `d`, numbered 1 through 4.
///
/// The Form 941 return periods of 26 CFR 31.6011(a)-1(a)(1): January through
/// March, April through June, July through September, October through December.
pub fn quarter_of(d: NaiveDate) -> u32 {
    (d.month() - 1) / 3 + 1
}

/// The last day of `quarter` in `year`: March 31, June 30, September 30, or
/// December 31.
///
/// # Panics
///
/// Panics when `quarter` is outside 1 through 4. A quarter number is derived
/// from a date or a command-line argument that has already been validated, so a
/// bad value here is a programming error rather than a payroll condition.
pub fn quarter_end(year: i32, quarter: u32) -> NaiveDate {
    let month = match quarter {
        1 => 3,
        2 => 6,
        3 => 9,
        4 => 12,
        other => panic!("quarter must be 1, 2, 3, or 4, got {other}"),
    };
    last_day_of_month(year, month)
}

/// The last day an employee's elective deferrals withheld from wages paid on
/// `pay_date` can reach the plan and still land inside the small-plan safe
/// harbor: the seventh business day following the pay date.
///
/// 29 CFR 2510.3-102(a)(2)(i). For a plan with fewer than 100 participants at
/// the beginning of the plan year, an amount withheld from a participant's wages
/// and "deposited with such plan not later than the 7th business day following
/// the day on which such amount would otherwise have been payable to the
/// participant in cash... shall be deemed to be contributed... on the earliest
/// date on which such contributions... can reasonably be segregated from the
/// employer's general assets."
///
/// Two cautions on what this date is not. The safe harbor is an optional
/// alternative method of compliance, not a due date: 2510.3-102(a)(2)(ii) is
/// explicit that it "does not establish the exclusive means" of satisfying the
/// general rule of 2510.3-102(a)(1), which requires segregation on the earliest
/// date reasonably possible. For a one-employee plan run off a single payroll
/// that earliest date is usually the pay date itself. Depositing on day seven is
/// safe; treating day seven as the target is not the same as complying with
/// paragraph (a)(1), only as being deemed to comply.
///
/// Business days here are the DOL's own, 2510.3-102(e): Saturdays, Sundays, and
/// federal holidays are skipped. District of Columbia holidays are not, because
/// they are not "designated as a holiday by the Federal Government".
pub fn deferral_remittance_due(holidays: &LegalHolidays, pay_date: NaiveDate) -> Result<NaiveDate> {
    add_business_days(holidays, pay_date, 7)
}

/// `d` shifted by `n` days, negative to go backwards.
///
/// Goes through the days-from-common-era count rather than a duration so the
/// month and year rollover is arithmetic rather than a special case.
fn add_days(d: NaiveDate, n: i32) -> NaiveDate {
    NaiveDate::from_num_days_from_ce_opt(d.num_days_from_ce() + n)
        .expect("payroll dates stay far inside the representable date range")
}

fn last_day_of_month(year: i32, month: u32) -> NaiveDate {
    let (next_year, next_month) = match month {
        12 => (year + 1, 1),
        month => (year, month + 1),
    };
    let first_of_next =
        NaiveDate::from_ymd_opt(next_year, next_month, 1).expect("the 1st of a real month");
    add_days(first_of_next, -1)
}
