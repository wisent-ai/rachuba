//! Line values for the federal returns, derived from the ledger.
//!
//! This module does not produce filled PDFs. Forms 941 and 940 are keyed into
//! EFTPS or the IRS e-file provider, and the W-2 is keyed into SSA Business
//! Services Online, all of which want typed values rather than a rendered page.
//! What it produces is the set of numbers to type, each labelled with its form
//! line, computed from the same ledger that produced the paychecks.

use crate::calendar::{deferral_remittance_due, employment_tax_deposit_due, DepositSchedule};
use crate::ledger::Ledger;
use crate::money::Cents;
use crate::tables::FederalTables;
use serde::Serialize;

mod form940;
mod form941;
mod w2;
pub use form940::Form940;
pub use form941::Form941;
pub use w2::FormW2;

/// A single labelled value to key into a form.
#[derive(Serialize)]
pub struct Line {
    pub number: &'static str,
    pub label: &'static str,
    pub value: Cents,
}

/// A limit or due date the ledger says has been breached or is approaching.
#[derive(Serialize)]
pub struct Finding {
    pub severity: Severity,
    pub message: String,
}

#[derive(PartialEq, Eq, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

impl Severity {
    pub fn tag(self) -> &'static str {
        match self {
            Severity::Error => "ERROR",
            Severity::Warning => "WARN",
            Severity::Info => "INFO",
        }
    }
}

/// Check the year's ledger against the statutory limits.
///
/// These are the failures that cost money: an excess deferral has to be
/// distributed by 15 April or it is taxed twice, and an over-limit annual
/// addition can disqualify the plan.
pub fn check_limits(
    ledger: &Ledger,
    tables: &FederalTables,
    year: i32,
    age: Option<u32>,
    prior_year_ss_wages: Cents,
) -> Vec<Finding> {
    let y = ledger.ytd(year);
    let r = &tables.retirement;
    let mut out = Vec::new();

    let ceiling = r.deferral_ceiling(age);
    let deferred = y.elective_deferral();
    if deferred > ceiling {
        out.push(Finding {
            severity: Severity::Error,
            message: format!(
                "elective deferrals of {deferred} exceed the {year} limit of {ceiling} by {}. \
                 The excess must be distributed by 15 April {} or it is taxed twice.",
                deferred - ceiling,
                year + 1
            ),
        });
    } else if deferred == ceiling {
        out.push(Finding {
            severity: Severity::Info,
            message: format!("elective deferrals have reached the {year} limit of {ceiling}."),
        });
    } else {
        out.push(Finding {
            severity: Severity::Info,
            message: format!(
                "elective deferrals {deferred} of {ceiling}; {} of room remains this year.",
                ceiling - deferred
            ),
        });
    }

    let additions = y.annual_additions();
    if additions > r.annual_additions_limit {
        out.push(Finding {
            severity: Severity::Error,
            message: format!(
                "annual additions of {additions} exceed the section 415(c) limit of {} by {}.",
                r.annual_additions_limit,
                additions - r.annual_additions_limit
            ),
        });
    }

    // The employer contribution is capped at 25% of compensation.
    let employer_cap = y.gross.mul_ppm(250_000);
    if y.employer_401k > employer_cap {
        out.push(Finding {
            severity: Severity::Error,
            message: format!(
                "employer contributions of {} exceed 25% of {} compensation ({}).",
                y.employer_401k, y.gross, employer_cap
            ),
        });
    }

    if prior_year_ss_wages > r.roth_catch_up_wage_threshold
        && r.catch_up_for_age(age).is_positive()
        && y.pretax_401k > r.elective_deferral_limit
    {
        out.push(Finding {
            severity: Severity::Error,
            message: format!(
                "prior-year Social Security wages of {prior_year_ss_wages} exceed {}, so every \
                 catch-up dollar must be designated Roth. Pre-tax deferrals above {} are not \
                 permitted.",
                r.roth_catch_up_wage_threshold, r.elective_deferral_limit
            ),
        });
    }

    if y.ss_wages >= tables.social_security.wage_base {
        out.push(Finding {
            severity: Severity::Info,
            message: format!(
                "Social Security wage base of {} reached; no further OASDI withholding this year.",
                tables.social_security.wage_base
            ),
        });
    }

    out
}

/// Runs of `year` whose money has not yet reached where it legally has to
/// be, each with the date it is due: the deferral by the plan's safe harbor,
/// the Form 941 taxes by the employer's deposit schedule. Either one is an
/// error once its date has passed.
pub fn outstanding_obligations(
    ledger: &Ledger,
    year: i32,
    schedule: DepositSchedule,
    today: chrono::NaiveDate,
) -> Vec<Finding> {
    let mut out = Vec::new();
    for r in ledger.runs_in_year(year) {
        if r.elective_deferral().is_positive() && r.deferral_remitted.is_none() {
            let due = deferral_remittance_due(r.pay_date);
            let (severity, verb) = past_or_coming(today, due);
            out.push(Finding {
                severity,
                message: format!(
                    "{} deferral from the {} payroll has not been remitted to the plan; it {verb} {due} \
                     under the seven-business-day safe harbor at 29 CFR 2510.3-102(a)(2).",
                    r.elective_deferral(),
                    r.pay_date
                ),
            });
        }
        if r.taxes_deposited.is_none() && r.form_941_liability().is_positive() {
            let due = employment_tax_deposit_due(schedule, r.pay_date);
            let (severity, verb) = past_or_coming(today, due);
            out.push(Finding {
                severity,
                message: format!(
                    "{} of Form 941 taxes from the {} payroll has not been recorded as deposited; \
                     it {verb} {due} through EFTPS.",
                    r.form_941_liability(),
                    r.pay_date
                ),
            });
        }
    }
    out
}

/// An obligation due on `due` is a warning until that day and an error after.
fn past_or_coming(today: chrono::NaiveDate, due: chrono::NaiveDate) -> (Severity, &'static str) {
    if today > due {
        (Severity::Error, "was due")
    } else {
        (Severity::Warning, "is due")
    }
}




