//! Method B and the separate employee and employer insurance calculations.

use super::{apply_rate, round_half_up, CaStatus, Tables};
use crate::config::PayFrequency;
use crate::money::Cents;
use crate::state::{StateInput, StateResult};
use anyhow::{bail, Context, Result};

/// DE 44 prints a table for every payroll period California recognises, so
/// unlike New York there is no frequency this engine has to refuse.
fn period_key(freq: PayFrequency) -> &'static str {
    match freq {
        PayFrequency::Daily => "daily",
        PayFrequency::Weekly => "weekly",
        PayFrequency::Biweekly => "biweekly",
        PayFrequency::Semimonthly => "semimonthly",
        PayFrequency::Monthly => "monthly",
        PayFrequency::Quarterly => "quarterly",
        PayFrequency::Semiannually => "semiannual",
        PayFrequency::Annually => "annual",
    }
}

/// Look up a ten-cell allowance table.
///
/// At or below ten allowances the printed cell is used, because the cells are
/// rounded from `n x annual / periods` and are not multiples of the first one.
/// Above ten, DE 44 switches rule: multiply the one-allowance figure by the
/// count. The two rules deliberately disagree at the seam, and the employer is
/// entitled to the published amount.
pub(super) fn allowance_amount(table: &[Cents], count: i64) -> Cents {
    if count <= 0 || table.is_empty() {
        Cents::ZERO
    } else if (count as usize) <= table.len() {
        table[count as usize - 1]
    } else {
        Cents(table[0].0 * count)
    }
}

/// The Method B computation, returning the tax before the flat DE 4 addition.
///
/// `None` means Step 1 short-circuited: gross was at or below the low income
/// exemption, so no computation occurred and even the requested flat addition
/// is suppressed.
pub(super) fn method_b(
    t: &Tables,
    status: CaStatus,
    gross: Cents,
    regular: i64,
    estimated: i64,
    period: &str,
    annualize: bool,
) -> Result<Option<Cents>> {
    let pick = status.column(regular);

    // On the annualized path every table is read at "annual" and the wage is
    // multiplied up to a year first. DE 44 page 45.
    let periods = *t
        .payroll_periods
        .get(period)
        .with_context(|| format!("payroll_periods has no entry for {period}"))?;
    let (period, gross) = if annualize {
        ("annual", Cents(gross.0 * periods))
    } else {
        (period, gross)
    };
    let period: &str = period;

    // Step 1: low income exemption. Compared against raw gross, inclusive, and
    // it short-circuits rather than reducing anything.
    let lie = pick(
        t.pit
            .low_income_exemption
            .get(period)
            .with_context(|| format!("pit.low_income_exemption has no entry for {period}"))?,
    );
    if gross <= lie {
        return Ok(None);
    }

    // Step 2: estimated deduction, from the SECOND allowance count.
    let wages = if estimated > 0 {
        let table = t
            .pit
            .estimated_deduction
            .get(period)
            .with_context(|| format!("pit.estimated_deduction has no entry for {period}"))?;
        gross - allowance_amount(table, estimated)
    } else {
        gross
    };

    // Step 3: standard deduction. Same column rule as Step 1.
    let standard = pick(
        t.pit
            .standard_deduction
            .get(period)
            .with_context(|| format!("pit.standard_deduction has no entry for {period}"))?,
    );
    let taxable = wages - standard;
    if taxable <= Cents::ZERO {
        // Only reachable when estimated deductions are claimed, since the
        // exemption always exceeds the standard deduction. California has no
        // refundable withholding, so this floors at zero.
        return Ok(Some(Cents::ZERO));
    }

    // Step 4: rate table, chosen by marital status alone.
    let rows = t
        .pit
        .brackets
        .get(period)
        .with_context(|| format!("pit.brackets has no entry for {period}"))?
        .pick(status);
    let row = rows
        .iter()
        .rev()
        .find(|r| taxable >= r.0)
        .copied()
        .unwrap_or((Cents::ZERO, Cents::ZERO, 0));
    // Round the rate product, then add the published base. DE 44's worked
    // examples print that intermediate, and the printed intermediate is what an
    // auditor checks.
    let computed = apply_rate(taxable - row.0, row.2) + row.1;

    // Step 5: the exemption allowance is a CREDIT against the computed tax, not
    // a deduction from wages, and is indexed by the regular count alone.
    let credit = if regular > 0 {
        let table = t
            .pit
            .exemption_allowance
            .get(period)
            .with_context(|| format!("pit.exemption_allowance has no entry for {period}"))?;
        allowance_amount(table, regular)
    } else {
        Cents::ZERO
    };

    // Clamp before the division: California has no refundable withholding, and
    // clamping first keeps the intermediate honest for a trace.
    let tax = (computed - credit).floor_zero();
    Ok(Some(if annualize {
        Cents(round_half_up(tax.0 as i128, periods as i128) as i64)
    } else {
        tax
    }))
}

pub fn compute(t: &Tables, input: &StateInput<'_>) -> Result<StateResult> {
    let e = input.employee;
    let period = period_key(e.pay_frequency);

    let status = match e.state_filing_status() {
        crate::config::StateFilingStatus::Single => CaStatus::Single,
        crate::config::StateFilingStatus::Married => CaStatus::Married,
        crate::config::StateFilingStatus::HeadOfHousehold => CaStatus::HeadOfHousehold,
    };

    let state_income_tax = match method_b(
        t,
        status,
        input.taxable_wages,
        e.state_allowances.max(0),
        e.state_estimated_allowances.max(0),
        period,
        input.company.state_annualized_method,
    )? {
        // Step 1 short-circuited: the flat addition is suppressed with it.
        None => Cents::ZERO,
        Some(tax) => tax + e.state_additional_withholding,
    };

    // California bars local income tax statewide. Assert it rather than assume.
    let local_income_tax = if t.local.employee_withholding {
        bail!("state-CA table claims a local employee withholding, which California law bars")
    } else {
        Cents::ZERO
    };

    // SDI: a flat rate on every dollar. SB 951 removed the taxable wage ceiling
    // effective 2024 and there is no maximum withholding, so there is no cap
    // term here and no year-to-date test. Paid Family Leave is funded from this
    // same contribution and is NOT a separate deduction.
    let sdi_employee = apply_rate(input.gross_wages, t.sdi.employee_rate_ppm);

    // Unemployment and the training tax, both employer-side, both on the first
    // $7,000 of wages.
    let sui_rate = input.company.sui_rate_ppm;
    if sui_rate < t.sui.assigned_rate_min_ppm || sui_rate > t.sui.assigned_rate_max_ppm {
        bail!(
            "company.sui_rate_ppm of {} is outside California's {} Schedule F+ range of {}-{} \
             ppm. Copy the rate from your EDD rate notice; the new-employer rate is {} ppm.",
            sui_rate,
            t.year,
            t.sui.assigned_rate_min_ppm,
            t.sui.assigned_rate_max_ppm,
            t.sui.new_employer_rate_ppm
        );
    }
    let sui_taxable = input
        .gross_wages
        .min((t.sui.wage_base - input.ytd_sui_wages).floor_zero());
    let ett_taxable = input
        .gross_wages
        .min((t.ett.wage_base - input.ytd_sui_wages).floor_zero());

    Ok(StateResult {
        state_income_tax,
        local_income_tax,
        sdi_employee,
        pfl_employee: Cents::ZERO,
        sui_employer: apply_rate(sui_taxable, sui_rate),
        training_tax_employer: apply_rate(ett_taxable, t.ett.rate_ppm),
    })
}
