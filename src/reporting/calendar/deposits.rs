//! Monthly and semiweekly employment-tax deposit due dates.

use super::holidays::{irs_business_day_on_or_after, is_irs_legal_holiday};
use super::{add_days, DepositSchedule};
use chrono::{Datelike, NaiveDate, Weekday};

/// When the employment taxes on wages paid on `pay_date` must be deposited.
///
/// Monthly depositors, 26 CFR 31.6302-1(c)(1): taxes accumulated on payments
/// made during a calendar month are due "by the 15th day of the following
/// month", and "if the 15th day of the following month is a Saturday, Sunday,
/// or legal holiday in the District of Columbia under section 7503, taxes will
/// be treated as timely deposited if deposited on the next succeeding day which
/// is not a Saturday, Sunday, or legal holiday."
///
/// Semiweekly depositors, 26 CFR 31.6302-1(c)(2)(i): wages paid Wednesday,
/// Thursday, and/or Friday are due on or before the following Wednesday; wages
/// paid Saturday, Sunday, Monday, and/or Tuesday on or before the following
/// Friday.
///
/// That scheduled date is then extended twice over. First by
/// 31.6302-1(c)(2)(iii): "Semi-weekly depositors have at least three business
/// days following the close of the semi-weekly period by which to deposit
/// employment taxes... If any of the three weekdays following the close of a
/// semi-weekly period is a legal holiday, the employer has an additional day
/// for each day that is a legal holiday by which to make the required
/// deposit." A plain next-business-day rule is not a substitute: it leaves the
/// Wednesday due date in place when the intervening Monday is a holiday, and
/// the regulation's own worked example says otherwise. Second by
/// 31.6302-1(c)(4), "Deposits required only on business days", which moves a
/// due date landing on a Saturday, Sunday, or legal holiday to the next
/// business day.
///
/// "Legal holiday" throughout is the section 7503 sense, so Emancipation Day
/// and Inauguration Day count. See `is_irs_legal_holiday`.
///
/// The semiweekly deposit period can straddle two return periods, in which case
/// the wages on either side of the boundary carry separate deposit obligations
/// with separate Schedule B lines. 26 CFR 31.6302-1(c)(2)(ii). Both obligations
/// share the due date this function returns, so callers split the liability by
/// quarter and call once per pay date.
pub fn employment_tax_deposit_due(schedule: DepositSchedule, pay_date: NaiveDate) -> NaiveDate {
    match schedule {
        DepositSchedule::Monthly => {
            let (year, month) = match pay_date.month() {
                12 => (pay_date.year() + 1, 1),
                month => (pay_date.year(), month + 1),
            };
            let scheduled =
                NaiveDate::from_ymd_opt(year, month, 15).expect("every month has a 15th");
            irs_business_day_on_or_after(scheduled)
        }
        DepositSchedule::Semiweekly => {
            // The deposit periods are Wednesday through Friday and Saturday
            // through Tuesday, so every pay date sits in a period that closes
            // on the coming Friday or the coming Tuesday.
            let period_close = match pay_date.weekday() {
                Weekday::Wed => add_days(pay_date, 2),
                Weekday::Thu => add_days(pay_date, 1),
                Weekday::Fri => pay_date,
                Weekday::Sat => add_days(pay_date, 3),
                Weekday::Sun => add_days(pay_date, 2),
                Weekday::Mon => add_days(pay_date, 1),
                Weekday::Tue => pay_date,
            };
            // A Friday close is due the following Wednesday, five days on; a
            // Tuesday close the following Friday, three days on.
            let scheduled = match period_close.weekday() {
                Weekday::Fri => add_days(period_close, 5),
                _ => add_days(period_close, 3),
            };
            // The three weekdays following the close are Monday through
            // Wednesday for a Friday close and Wednesday through Friday for a
            // Tuesday close. Either way they are the scheduled date and the two
            // weekdays before it, so counting back from the scheduled date
            // covers both bands without a second weekday match.
            let intervening_holidays = (0i32..3)
                .filter(|&back| is_irs_legal_holiday(add_days(scheduled, -back)))
                .count();
            irs_business_day_on_or_after(add_days(scheduled, intervening_holidays as i32))
        }
    }
}
