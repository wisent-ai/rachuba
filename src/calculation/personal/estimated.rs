//! The estimated tax safe harbor, installment by installment.
//!
//! Section 6654 charges an addition to tax, interest by another name, on
//! every installment that was short when it fell due. Each of the four
//! installments is a quarter of the required annual payment: the lesser of
//! 90% of this year's tax and 100% of last year's, or 110% when last year's
//! adjusted gross income was above the threshold. Without last year's figures
//! only the 90% rule is available.
//!
//! Withholding counts as paid in four equal parts on the four due dates
//! whenever during the year it was actually withheld, section 6654(g), which
//! is why a late-year increase in withholding can cover an early shortfall and
//! an estimated payment cannot. Estimated payments count from the day paid.
//!
//! The annualized income installment method of section 6654(d)(2), which
//! helps when income arrived late in the year, is not computed here: the
//! shortfalls shown are what the regular method charges, the most it can be.

use chrono::NaiveDate;
use serde::Serialize;

use super::EstimatedTaxRules;
use crate::calendar::{estimated_tax_due, ESTIMATED_TAX_INSTALLMENTS};
use crate::config::EstimatedPayment;
use crate::money::{Cents, WHOLE_PPM};
use crate::tables::FilingStatus;

#[derive(Clone, Copy, Debug, Serialize)]
pub struct Installment {
    pub number: u32,
    pub due: NaiveDate,
    /// Cumulative required payment by this due date.
    pub required_to_date: Cents,
    /// Withholding allocated to date plus estimated payments made by the due
    /// date.
    pub credited_to_date: Cents,
    pub shortfall: Cents,
}

#[derive(Clone, Debug, Serialize)]
pub struct EstimatedTax {
    pub required_annual_payment: Cents,
    /// 90% of this year's tax.
    pub current_year_share: Cents,
    /// 100% or 110% of last year's tax, when last year's return was given.
    pub prior_year_share: Option<Cents>,
    pub prior_year_ppm: Option<i64>,
    pub withholding: Cents,
    pub estimated_payments: Cents,
    /// Tax less withholding is under the threshold, so no addition applies.
    pub below_minimum: bool,
    pub installments: Vec<Installment>,
    /// The first installment due on or after today, if the year has one left.
    pub next_due: Option<Installment>,
    /// Total tax less withholding and every estimated payment; negative is a
    /// refund.
    pub balance_due: Cents,
}

pub struct EstimatedInput<'a> {
    pub rules: &'a EstimatedTaxRules,
    pub status: FilingStatus,
    pub year: i32,
    pub total_tax: Cents,
    pub withholding: Cents,
    pub payments: &'a [EstimatedPayment],
    /// Last year's (tax, adjusted gross income).
    pub prior_year: Option<(Cents, Cents)>,
    pub today: NaiveDate,
}

pub fn estimated_tax(i: &EstimatedInput<'_>) -> EstimatedTax {
    let r = i.rules;
    let current_year_share = i.total_tax.mul_ppm(r.current_year_ppm);
    let (prior_year_share, prior_year_ppm) = match i.prior_year {
        Some((tax, agi)) => {
            let threshold = if i.status == FilingStatus::MarriedFilingSeparately {
                r.high_income_agi_separate
            } else {
                r.high_income_agi
            };
            let ppm = if agi > threshold {
                r.prior_year_high_income_ppm
            } else {
                r.prior_year_ppm
            };
            (Some(tax.mul_ppm(ppm)), Some(ppm))
        }
        None => (None, None),
    };
    let required_annual_payment = match prior_year_share {
        Some(prior) => prior.min(current_year_share),
        None => current_year_share,
    };
    let below_minimum = i.total_tax - i.withholding < r.minimum_balance_due;

    let installments: Vec<Installment> = (1..=ESTIMATED_TAX_INSTALLMENTS)
        .map(|n| {
            let due = estimated_tax_due(i.year, n);
            let share = WHOLE_PPM * i64::from(n) / i64::from(ESTIMATED_TAX_INSTALLMENTS);
            let required_to_date = required_annual_payment.mul_ppm(share);
            let paid_by_due: Cents = i
                .payments
                .iter()
                .filter(|p| p.paid_on <= due)
                .map(|p| p.amount)
                .sum();
            let credited_to_date = i.withholding.mul_ppm(share) + paid_by_due;
            let shortfall = if below_minimum {
                Cents::ZERO
            } else {
                (required_to_date - credited_to_date).floor_zero()
            };
            Installment {
                number: n,
                due,
                required_to_date,
                credited_to_date,
                shortfall,
            }
        })
        .collect();
    let next_due = installments.iter().find(|x| x.due >= i.today).copied();
    let estimated_payments: Cents = i.payments.iter().map(|p| p.amount).sum();

    EstimatedTax {
        required_annual_payment,
        current_year_share,
        prior_year_share,
        prior_year_ppm,
        withholding: i.withholding,
        estimated_payments,
        below_minimum,
        installments,
        next_due,
        balance_due: i.total_tax - i.withholding - estimated_payments,
    }
}
