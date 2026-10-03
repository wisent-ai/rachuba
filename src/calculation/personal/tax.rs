//! Form 1040 tax for one year's income.
//!
//! The order is the return's own:
//!
//! 1. Capital gains and losses are netted. A net loss offsets at most the
//!    section 1211(b) limit of other income; a net gain is ordinary income
//!    except for the part that is net long-term gain, which with qualified
//!    dividends is taxed at the 0, 15 and 20 percent rates.
//! 2. Adjusted gross income, less the larger of the standard deduction and the
//!    itemized deductions, is taxable income.
//! 3. Income tax is the lesser of the rate schedule on all taxable income and
//!    the Qualified Dividends and Capital Gain Tax Worksheet, which stacks the
//!    preferential income on top of the ordinary income.
//! 4. Nonrefundable credits reduce income tax, never below zero.
//! 5. The net investment income tax and the Additional Medicare Tax are added.
//!
//! The rate schedule is used at every income level. Below $100,000 of taxable
//! income the printed Tax Table rounds to the middle of a $50 band and can
//! differ from the schedule by a few dollars; that is a presentation of the
//! same law, not a different tax.

use serde::Serialize;

use super::IndividualTables;
use crate::money::Cents;
use crate::tables::FilingStatus;

/// One year's income as the return sees it.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct ReturnIncome {
    pub status: FilingStatus,
    /// Box 1 of every Form W-2 on the return.
    pub wages: Cents,
    /// Medicare wages of every Form W-2 on the return, for Form 8959.
    pub medicare_wages: Cents,
    /// Ordinary income that is neither wages nor investment income.
    pub other_income: Cents,
    /// Interest and dividends that are not qualified: ordinary investment
    /// income.
    pub interest_and_ordinary_dividends: Cents,
    pub qualified_dividends: Cents,
    pub short_term_capital_gains: Cents,
    pub long_term_capital_gains: Cents,
    /// Adjustments to income (Schedule 1, Part II).
    pub adjustments: Cents,
    pub itemized_deductions: Cents,
    /// Boxes checked for age 65 or blindness, for the filer and a spouse.
    pub aged_or_blind_boxes: u32,
    /// Nonrefundable credits.
    pub credits: Cents,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct TaxComputation {
    /// Net capital gain or deductible loss included in income.
    pub capital_gain_in_income: Cents,
    pub adjusted_gross_income: Cents,
    pub deduction: Cents,
    /// "standard" or "itemized".
    pub deduction_kind: &'static str,
    pub taxable_income: Cents,
    /// Qualified dividends and net long-term gain taxed at capital gains rates.
    pub preferential_income: Cents,
    /// Income tax before credits.
    pub income_tax: Cents,
    pub credits_applied: Cents,
    pub net_investment_income: Cents,
    pub net_investment_income_tax: Cents,
    pub additional_medicare_tax: Cents,
    pub total_tax: Cents,
    /// Rate of the ordinary bracket the last dollar of ordinary income falls in.
    pub ordinary_bracket_ppm: i64,
}

/// The year's federal tax on `i`, every rate and threshold read from `t`.
pub fn compute_tax(t: &IndividualTables, i: &ReturnIncome) -> TaxComputation {
    let status = i.status;

    // 1. Capital gains.
    let net_capital = i.short_term_capital_gains + i.long_term_capital_gains;
    let loss_limit = match status {
        FilingStatus::MarriedFilingSeparately => t.capital_loss_limit.married_filing_separately,
        _ => t.capital_loss_limit.other,
    };
    let (capital_gain_in_income, net_capital_gain) = if net_capital < Cents::ZERO {
        (max(net_capital, -loss_limit), Cents::ZERO)
    } else {
        (
            net_capital,
            i.long_term_capital_gains.min(net_capital).floor_zero(),
        )
    };

    // 2. Adjusted gross income, deduction, taxable income.
    let adjusted_gross_income = i.wages
        + i.other_income
        + i.interest_and_ordinary_dividends
        + i.qualified_dividends
        + capital_gain_in_income
        - i.adjustments;
    let per_box = match status {
        FilingStatus::Single | FilingStatus::HeadOfHousehold => {
            t.standard_deduction.aged_or_blind_unmarried
        }
        _ => t.standard_deduction.aged_or_blind_married,
    };
    let standard = *t.standard_deduction.basic.get(status)
        + Cents(per_box.0 * i64::from(i.aged_or_blind_boxes));
    let (deduction, deduction_kind) = if i.itemized_deductions > standard {
        (i.itemized_deductions, "itemized")
    } else {
        (standard, "standard")
    };
    let taxable_income = (adjusted_gross_income - deduction).floor_zero();

    // 3. Income tax: the schedule, or the capital gains worksheet when lower.
    let schedule = t.rates.get(status);
    let preferential_income = (net_capital_gain + i.qualified_dividends)
        .min(taxable_income)
        .floor_zero();
    let ordinary = taxable_income - preferential_income;
    let on_schedule = schedule.tax_on(taxable_income);
    let income_tax = if preferential_income.is_positive() {
        let cg = &t.capital_gains;
        let (zero_max, fifteen_max) = *cg.thresholds.get(status);
        // Worksheet lines 6 to 9: the part of the preferential income that
        // fits under the zero rate amount once ordinary income is stacked.
        let at_zero = taxable_income.min(zero_max) - ordinary.min(taxable_income.min(zero_max));
        let above_zero = preferential_income - at_zero;
        // Lines 13 to 17: the part that fits under the 15 percent amount.
        let room_fifteen = (taxable_income.min(fifteen_max) - (ordinary + at_zero)).floor_zero();
        let at_fifteen = above_zero.min(room_fifteen);
        let at_twenty = above_zero - at_fifteen;
        let worksheet = schedule.tax_on(ordinary)
            + at_fifteen.mul_ppm(cg.middle_rate_ppm)
            + at_twenty.mul_ppm(cg.top_rate_ppm);
        worksheet.min(on_schedule)
    } else {
        on_schedule
    };

    // 4. Credits.
    let credits_applied = i.credits.min(income_tax).floor_zero();

    // 5. Net investment income tax and Additional Medicare Tax.
    let net_investment_income = (i.interest_and_ordinary_dividends
        + i.qualified_dividends
        + capital_gain_in_income)
        .floor_zero();
    let niit = &t.net_investment_income_tax;
    let over_threshold = (adjusted_gross_income - *niit.threshold.get(status)).floor_zero();
    let net_investment_income_tax = net_investment_income
        .min(over_threshold)
        .mul_ppm(niit.rate_ppm);
    let medicare = &t.additional_medicare;
    let additional_medicare_tax = (i.medicare_wages - *medicare.threshold.get(status))
        .floor_zero()
        .mul_ppm(medicare.rate_ppm);

    let total_tax =
        income_tax - credits_applied + net_investment_income_tax + additional_medicare_tax;

    TaxComputation {
        capital_gain_in_income,
        adjusted_gross_income,
        deduction,
        deduction_kind,
        taxable_income,
        preferential_income,
        income_tax,
        credits_applied,
        net_investment_income,
        net_investment_income_tax,
        additional_medicare_tax,
        total_tax,
        ordinary_bracket_ppm: schedule.bracket_for(ordinary).rate_ppm,
    }
}

fn max(a: Cents, b: Cents) -> Cents {
    if a >= b {
        a
    } else {
        b
    }
}
