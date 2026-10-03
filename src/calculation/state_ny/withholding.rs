//! Exact state and city withholding and published per-period table derivation.

use super::{
    round_half_even, round_half_up, AnnualSchedule, Exact, Exclusion, NyStatus, PeriodOverride,
    Tables,
};
use crate::config::PayFrequency;
use crate::money::Cents;
use anyhow::{bail, Context, Result};

// ---------------------------------------------------------------------------
// Method II and Method III
// ---------------------------------------------------------------------------

/// The publication's key for a payroll period.
///
/// New York publishes deduction and exemption tables for five periods plus
/// annual. Quarterly and semiannual payroll have no published New York figures,
/// so they are refused rather than approximated.
pub(super) fn period_key(freq: PayFrequency) -> Result<&'static str> {
    Ok(match freq {
        PayFrequency::Daily => "daily",
        PayFrequency::Weekly => "weekly",
        PayFrequency::Biweekly => "biweekly",
        PayFrequency::Semimonthly => "semimonthly",
        PayFrequency::Monthly => "monthly",
        PayFrequency::Annually => "annual",
        PayFrequency::Quarterly | PayFrequency::Semiannually => bail!(
            "New York publishes no deduction or exemption allowance for {} payroll. Pay \
             monthly or more often, or compute this period by hand from NYS-50-T-NYS.",
            freq.label()
        ),
    })
}

/// Derive one payroll period's schedule from the annual one, per the rule that
/// reproduces every printed row: thresholds round half to even at the period's
/// quantum, base amounts round half to even to the cent, rates pass through.
pub(super) fn derive_schedule(
    annual: &AnnualSchedule,
    periods: i64,
    quantum: Cents,
    overrides: &[PeriodOverride],
    period: &str,
) -> Vec<(Cents, Cents, i64)> {
    let n = periods as i128;
    let q = quantum.0.max(1) as i128;
    let mut rows: Vec<(Cents, Cents, i64)> = annual
        .rows
        .iter()
        .map(|&(at_least, base, rate)| {
            (
                Cents((round_half_even(at_least.0 as i128, n * q) * q) as i64),
                Cents(round_half_even(base.0 as i128, n) as i64),
                rate,
            )
        })
        .collect();

    for o in overrides.iter().filter(|o| o.period == period) {
        if let Some(row) = rows.get_mut(o.row.saturating_sub(1)) {
            if let Some(v) = o.at_least {
                row.0 = v;
            }
            if let Some(v) = o.base_tax {
                row.1 = v;
            }
        }
    }
    rows
}

/// Method II: find the row and build the exact tax as a rational.
fn method_ii(rows: &[(Cents, Cents, i64)], net: Cents) -> Exact {
    let row =
        rows.iter()
            .rev()
            .find(|r| net >= r.0)
            .copied()
            .unwrap_or((Cents::ZERO, Cents::ZERO, 0));
    Exact {
        num: row.1 .0 as i128 * 1_000_000 + (net.0 - row.0 .0) as i128 * row.2 as i128,
        den: 1_000_000,
    }
}

/// Method III: a flat rate on the entire annualized wage, divided back down to
/// the payroll period inside the denominator so that the division and the
/// rounding happen exactly once, together.
fn method_iii(rows: &[(Cents, i64)], annualized_net: Cents, periods: i64) -> Exact {
    let rate = rows
        .iter()
        .rev()
        .find(|r| annualized_net >= r.0)
        .map(|r| r.1)
        .unwrap_or(0);
    Exact {
        num: annualized_net.0 as i128 * rate as i128,
        den: 1_000_000 * periods as i128,
    }
}

/// The New York State tax, unrounded, so the Yonkers surcharge can be composed
/// onto it exactly.
pub(super) fn nys_exact(
    t: &Tables,
    status: NyStatus,
    wages: Cents,
    allowances: i64,
    period: &str,
) -> Result<Exact> {
    let n = *t
        .payroll_periods
        .get(period)
        .with_context(|| format!("payroll_periods has no entry for {period}"))?;

    let deduction = status.pick(
        *t.nys
            .deduction_allowance
            .get(period)
            .with_context(|| format!("nys.deduction_allowance has no entry for {period}"))?,
    );
    let exemption = *t
        .nys
        .exemption_allowance
        .get(period)
        .with_context(|| format!("nys.exemption_allowance has no entry for {period}"))?;

    let net = (wages - deduction - Cents(exemption.0 * allowances)).floor_zero();

    // The one place New York annualizes anything: the Method III test.
    let annualized = Cents(net.0 * n);
    let threshold = match status {
        NyStatus::Single => t.nys.method_iii_threshold_single,
        NyStatus::Married => t.nys.method_iii_threshold_married,
    };
    if annualized >= threshold {
        let rows = match status {
            NyStatus::Single => &t.nys.top_rates.single,
            NyStatus::Married => &t.nys.top_rates.married,
        };
        return Ok(method_iii(rows, annualized, n));
    }

    let quantum = *t
        .nys
        .period_threshold_quantum
        .get(period)
        .unwrap_or(&Cents(100));
    let rows = derive_schedule(t.nys.brackets.pick(status), n, quantum, &[], period);
    Ok(method_ii(&rows, net))
}

/// New York City resident tax. Same shape as the state, minus Method III, plus
/// the two published rows the derivation rule does not reproduce.
pub(super) fn nyc_exact(
    t: &Tables,
    status: NyStatus,
    wages: Cents,
    allowances: i64,
    period: &str,
) -> Result<Exact> {
    let n = *t
        .payroll_periods
        .get(period)
        .with_context(|| format!("payroll_periods has no entry for {period}"))?;

    let deduction = status.pick(
        *t.nyc
            .deduction_allowance
            .get(period)
            .with_context(|| format!("nyc.deduction_allowance has no entry for {period}"))?,
    );
    let exemption = *t
        .nyc
        .exemption_allowance
        .get(period)
        .with_context(|| format!("nyc.exemption_allowance has no entry for {period}"))?;

    let net = (wages - deduction - Cents(exemption.0 * allowances)).floor_zero();
    let quantum = *t
        .nyc
        .period_threshold_quantum
        .get(period)
        .unwrap_or(&Cents(100));
    let rows = derive_schedule(
        t.nyc.brackets.pick(status),
        n,
        quantum,
        &t.nyc.period_table_overrides,
        period,
    );
    Ok(method_ii(&rows, net))
}

/// Yonkers nonresident earnings tax, Method VIII.
///
/// The exclusion table's bounds are "Over" and "But not over", so the lower
/// bound is exclusive: the first threshold prints as 3,999.99 rather than
/// 4,000.00 and the row is selected with a strict comparison.
pub(super) fn yonkers_nonresident(t: &Tables, gross: Cents, periods: i64) -> Cents {
    let annualized = Cents(gross.0 * periods);
    let row = t
        .yonkers
        .nonresident_exclusions
        .rows
        .iter()
        .rev()
        .find(|r| annualized > r.0);
    let exclusion = match row.map(|r| &r.1) {
        Some(Exclusion::Amount(c)) => *c,
        // Either below the first threshold or explicitly no withholding.
        _ => return Cents::ZERO,
    };
    let taxable = (annualized - exclusion).floor_zero();
    Cents(round_half_up(
        taxable.0 as i128 * t.yonkers.nonresident_earnings_rate_ppm as i128,
        1_000_000 * periods as i128,
    ) as i64)
}
