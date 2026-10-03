//! New York State, New York City and Yonkers withholding.
//!
//! Implements the Exact Calculation Method (Method II) of NYS-50-T-NYS, the
//! top-rate method (Method III) above the annualized threshold, the identically
//! shaped New York City resident method of NYS-50-T-NYC, the Yonkers resident
//! surcharge and nonresident earnings tax of NYS-50-T-Y, and the statutory
//! disability and paid family leave contributions.
//!
//! Three properties of this method are unusual enough to state up front,
//! because each one breaks an assumption carried over from the federal engine:
//!
//! * **Wages are not annualized.** Method II de-annualizes the *table* instead,
//!   deriving each payroll period's schedule from the annual one. Annualization
//!   happens in exactly one place: the test that decides whether Method III
//!   applies.
//! * **The rate column is not monotonic.** New York's single schedule runs
//!   7.53%, 6.40%, 11.44%, 7.35%. A "rates must increase" assertion rejects
//!   valid published data, so no such assertion exists here.
//! * **The Yonkers surcharge multiplies the unrounded state figure.** Rounding
//!   the state tax to the cent first and then taking 16.75% of it produces a
//!   different answer, and the publication's own Example 3 shows the rounded
//!   path is wrong by a cent. The state tax is therefore carried as an exact
//!   rational until the last possible moment.

mod tables;
mod withholding;

pub use tables::{
    AnnualSchedule, BracketSchedules, ByStatus, Exclusion, NonresidentExclusions, NyStatus, Nyc,
    Nys, PeriodOverride, Pfl, Sdi, Sui, Tables, TopRateSchedules, TopRates, Yonkers,
};
use withholding::{nyc_exact, nys_exact, period_key, yonkers_nonresident};

use anyhow::{bail, Context, Result};

use crate::money::Cents;
use crate::state::{StateInput, StateResult};

/// An exact tax figure, kept unrounded so a surcharge can be composed onto it.
#[derive(Clone, Copy, Debug)]
struct Exact {
    num: i128,
    den: i128,
}

impl Exact {
    fn to_cents(self) -> Cents {
        Cents(round_half_up(self.num, self.den) as i64)
    }
}

/// Round `a / b` half up. Used for every published figure: it matches every
/// rounding shown in the publications' worked examples.
fn round_half_up(a: i128, b: i128) -> i128 {
    debug_assert!(b > 0);
    if a >= 0 {
        (2 * a + b) / (2 * b)
    } else {
        -((-2 * a + b) / (2 * b))
    }
}

/// Round `a / b` half to even. Used only to derive per-period schedule rows
/// from the annual schedule, where it reproduces all 170 printed rows.
fn round_half_even(a: i128, b: i128) -> i128 {
    debug_assert!(b > 0);
    debug_assert!(a >= 0);
    let q = a / b;
    let r = a - q * b;
    if 2 * r > b {
        q + 1
    } else if 2 * r == b {
        q + (q % 2)
    } else {
        q
    }
}

fn apply_rate(amount: Cents, rate_ppm: i64) -> Cents {
    Cents(round_half_up(amount.0 as i128 * rate_ppm as i128, 1_000_000) as i64)
}

pub fn compute(t: &Tables, input: &StateInput<'_>) -> Result<StateResult> {
    let e = input.employee;
    let period = period_key(e.pay_frequency)?;
    let n = *t
        .payroll_periods
        .get(period)
        .with_context(|| format!("payroll_periods has no entry for {period}"))?;

    // New York has no head-of-household withholding status.
    let status = match e.w4.filing_status {
        crate::tables::FilingStatus::MarriedFilingJointly => NyStatus::Married,
        _ => NyStatus::Single,
    };
    let allowances = e.state_allowances.max(0);

    let state_exact = nys_exact(t, status, input.taxable_wages, allowances, period)?;
    // Form IT-2104's requested flat extra is added after rounding; it is an
    // employee request, not part of the tax computation.
    let state_income_tax = state_exact.to_cents() + e.state_additional_withholding;

    // Local: New York City residency and Yonkers residency are mutually
    // exclusive, and the Yonkers nonresident tax applies only to a nonresident
    // earning in Yonkers.
    let mut local = Cents::ZERO;
    if e.has_flag("nyc_resident") {
        local += nyc_exact(t, status, input.taxable_wages, allowances, period)?.to_cents();
    }
    if e.has_flag("yonkers_resident") {
        // The surcharge multiplies the UNROUNDED state figure. Composing it
        // onto the rounded one is off by a cent on the publication's own
        // example, and the widest product on this path overflows 64 bits.
        local += Cents(round_half_up(
            state_exact.num * t.yonkers.resident_surcharge_rate_ppm as i128,
            state_exact.den * 1_000_000,
        ) as i64);
    } else if e.has_flag("yonkers_nonresident_earner") {
        local += yonkers_nonresident(t, input.gross_wages, n);
    }

    // Disability: half a percent of wages, capped at sixty cents a week, with
    // the weekly cap prorated across the payroll period and a year-to-date
    // clamp so proration rounding can never exceed fifty-two weekly caps.
    let annual_sdi_cap = Cents(t.sdi.employee_weekly_cap.0 * 52);
    let period_sdi_cap = Cents(round_half_up(annual_sdi_cap.0 as i128, n as i128) as i64);
    let sdi_employee = apply_rate(input.gross_wages, t.sdi.employee_rate_ppm)
        .min(period_sdi_cap)
        .min((annual_sdi_cap - input.ytd_sdi).floor_zero());

    // Paid family leave: a flat rate with an annual cap and no wage base. The
    // per-period stream need not reach the cap exactly, and is not trued up.
    let pfl_employee = apply_rate(input.gross_wages, t.pfl.employee_rate_ppm)
        .min((t.pfl.employee_annual_cap - input.ytd_pfl).floor_zero());

    // Unemployment: employer only, on gross wages up to the state wage base.
    let sui_rate = input.company.sui_rate_ppm;
    if sui_rate < t.sui.assigned_rate_min_ppm || sui_rate > t.sui.assigned_rate_max_ppm {
        bail!(
            "company.sui_rate_ppm of {} is outside New York's {} assigned range of {}-{} ppm. \
             Copy the rate from your Department of Labor rate notice; a value outside the band \
             is a data-entry error, not a rate.",
            sui_rate,
            t.year,
            t.sui.assigned_rate_min_ppm,
            t.sui.assigned_rate_max_ppm
        );
    }
    let sui_taxable = input
        .gross_wages
        .min((t.sui.wage_base - input.ytd_sui_wages).floor_zero());
    let sui_employer = apply_rate(sui_taxable, sui_rate);

    Ok(StateResult {
        state_income_tax,
        local_income_tax: local,
        sdi_employee,
        pfl_employee,
        sui_employer,
        // New York has no analogue of California's training tax.
        training_tax_employer: Cents::ZERO,
    })
}




