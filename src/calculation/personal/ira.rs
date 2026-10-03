//! How much one person may deduct for a traditional IRA and contribute to a
//! Roth IRA in a year.
//!
//! Both limits start from the same dollar amount, the IRA limit plus the
//! catch-up from age 50, capped at the compensation available to the person
//! (on a joint return that includes the spouse's, section 219(c)). Each then
//! phases out linearly across its income range. Inside a range the reduced
//! limit is rounded up to the next $10 and never falls below $200 until the
//! range is passed, section 219(g)(2)(B) and (C).
//!
//! Which deduction range applies turns on workplace plan coverage: the
//! person's own, or failing that the spouse's. Someone neither covered nor
//! married to someone covered deducts the full amount at any income. The Roth
//! range ignores coverage.

use serde::Serialize;

use super::IraTables;
use crate::money::Cents;
use crate::tables::FilingStatus;

/// One person's IRA position for the year.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct IraEligibility {
    /// The IRA limit for this person before any phase-out.
    pub limit: Cents,
    /// Whether the person is an active participant in a workplace plan.
    pub covered_by_plan: bool,
    /// The deductible traditional IRA contribution.
    pub traditional_deduction: Cents,
    /// The range the deduction was read against, when one applies.
    pub deduction_range: Option<(Cents, Cents)>,
    /// The Roth IRA contribution allowed directly.
    pub roth_contribution: Cents,
    pub roth_range: (Cents, Cents),
}

pub struct IraPerson {
    pub age_this_year: Option<u32>,
    pub covered_by_plan: bool,
    pub spouse_covered_by_plan: bool,
    /// Compensation this person may contribute against.
    pub compensation: Cents,
}

pub fn eligibility(
    t: &IraTables,
    status: FilingStatus,
    magi: Cents,
    person: &IraPerson,
) -> IraEligibility {
    let catch_up = match person.age_this_year {
        Some(age) if age >= t.catch_up_age => t.catch_up,
        _ => Cents::ZERO,
    };
    let limit = (t.contribution_limit + catch_up)
        .min(person.compensation)
        .floor_zero();

    let deduction_range = if person.covered_by_plan {
        Some(*t.deduction_covered.get(status))
    } else if person.spouse_covered_by_plan {
        match status {
            FilingStatus::MarriedFilingJointly => {
                Some(t.deduction_spouse_covered.married_filing_jointly)
            }
            FilingStatus::MarriedFilingSeparately => {
                Some(t.deduction_spouse_covered.married_filing_separately)
            }
            FilingStatus::Single | FilingStatus::HeadOfHousehold => None,
        }
    } else {
        None
    };
    let traditional_deduction = match deduction_range {
        Some(range) => reduced(t, limit, magi, range),
        None => limit,
    };
    let roth_range = *t.roth.get(status);

    IraEligibility {
        limit,
        covered_by_plan: person.covered_by_plan,
        traditional_deduction,
        deduction_range,
        roth_contribution: reduced(t, limit, magi, roth_range),
        roth_range,
    }
}

/// `limit` phased out across `(begins, complete)` at `magi`.
fn reduced(t: &IraTables, limit: Cents, magi: Cents, (begins, complete): (Cents, Cents)) -> Cents {
    if magi <= begins {
        return limit;
    }
    if magi >= complete {
        return Cents::ZERO;
    }
    // limit x (complete - magi) / (complete - begins), rounded up to the cent
    // and then up to the rounding increment.
    let num = i128::from(limit.0) * i128::from((complete - magi).0);
    let den = i128::from((complete - begins).0);
    let exact = (num + den - 1) / den;
    let step = i128::from(t.rounding_increment.0);
    let rounded = Cents((((exact + step - 1) / step) * step) as i64);
    let floored = if rounded < t.minimum_reduced_limit {
        t.minimum_reduced_limit
    } else {
        rounded
    };
    floored.min(limit)
}
