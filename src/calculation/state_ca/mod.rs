//! California withholding: Method B, the Exact Calculation Method.
//!
//! Implements DE 44's five-step Method B, the SDI employee contribution, and
//! the employer-side Unemployment Insurance and Employment Training Tax.
//!
//! Four properties break assumptions carried over from the federal and New York
//! engines, and each is a documented way to get California wrong:
//!
//! * **Two allowance counts that never mix.** Form DE 4 line 1 is regular
//!   allowances; line 2 is additional allowances for estimated deductions. The
//!   first drives the Table 1 and Table 3 column and the Table 4 credit; the
//!   second drives the Table 2 deduction and nothing else. DE 44 page 43
//!   footnote 1 is explicit that estimated-deduction allowances "must not be
//!   used in the determination of tax credits to be subtracted."
//! * **The low income exemption is a short circuit, not a deduction.** Gross at
//!   or below the Table 1 figure withholds nothing at all. One cent above it,
//!   the full computation runs on the entire gross and the exemption is
//!   subtracted from nothing. That cliff is published; it is not smoothed here.
//! * **Tables 1 and 3 are not keyed by filing status.** Their married column
//!   splits on the regular allowance count, while the rate table is chosen by
//!   marital status alone. A married employee with one allowance therefore
//!   takes single-sized deduction figures and the married rate table.
//! * **The exemption allowance is a credit against tax, not a deduction from
//!   wages,** and above ten allowances it switches from the printed row to a
//!   multiple of the one-allowance figure, which is deliberately not the same
//!   number.
//!
//! DE 44 sanctions two paths and they disagree by a few cents, because the
//! per-period tables are rounded independently of the annual one. The direct
//! path reads the table California prints for the employee's actual payroll
//! period. The annualized path of DE 44 page 45, used by Examples E and F,
//! multiplies wages up to a year, reads every table at annual, and divides the
//! result back down at the end.
//!
//! This engine defaults to the **annualized** path, for two reasons. Twelve
//! annualized payrolls sum to exactly the annual liability, where twelve
//! direct-path payrolls do not, so the employee's return reconciles. And it is
//! what commercial payroll providers compute, so switching to this engine
//! mid-year does not put a few-cent step in the year-to-date figures. Either
//! way it must be one policy held for the whole year: `Company::
//! state_annualized_method` sets it, and changing it mid-year is what breaks
//! reconciliation, not the choice itself.

mod tables;
mod withholding;

pub use tables::{
    ByColumn, CaStatus, Ett, FutaCreditReduction, Local, Pit, Sdi, StatusBrackets, Sui, Tables,
};
pub use withholding::compute;

use crate::money::Cents;

fn round_half_up(a: i128, b: i128) -> i128 {
    debug_assert!(b > 0);
    if a >= 0 {
        (2 * a + b) / (2 * b)
    } else {
        -((-2 * a + b) / (2 * b))
    }
}

fn apply_rate(amount: Cents, rate_ppm: i64) -> Cents {
    Cents(round_half_up(amount.0 as i128 * rate_ppm as i128, 1_000_000) as i64)
}

/// Warn when the configured FUTA credit reduction differs from the final
/// Schedule A rate, or when the current year's rate is still undetermined.
pub fn futa_advisory(t: &Tables, configured_ppm: i64) -> Option<String> {
    if let Some((_, rate)) = t
        .futa_credit_reduction
        .determined
        .iter()
        .find(|(year, _)| *year == t.year)
    {
        if *rate == configured_ppm {
            return None;
        }
        return Some(format!(
            "California's {} FUTA credit reduction is {} ppm in Schedule A (Form 940), \
             but company.futa_credit_reduction_ppm is {} ppm. Update the configuration \
             and revisit the year's Form 940 liability.",
            t.year, rate, configured_ppm
        ));
    }
    Some(format!(
        "California's {} FUTA credit reduction is not determined until {}. \
         company.futa_credit_reduction_ppm is {} ppm{}; verify the final rate in \
         Schedule A (Form 940) and revisit the year's liability.",
        t.year,
        t.futa_credit_reduction.test_date,
        configured_ppm,
        if configured_ppm == 0 {
            ", so no reduction is being accrued"
        } else {
            ", which is an estimate"
        }
    ))
}




