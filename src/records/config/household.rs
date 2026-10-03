//! The employee's own return: filing status and the income the payroll
//! never sees.
//!
//! Optional. Payroll runs ignore it; `rachuba return` refuses to run without
//! it. Every amount is for the whole calendar year, the best figure known
//! today.

use anyhow::{bail, Result};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::money::Cents;
use crate::tables::FilingStatus;

/// Boxes for age 65 or blindness: two per person.
const AGED_OR_BLIND_BOXES_PER_PERSON: u32 = 2;

/// A Form 1040-ES payment already made.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EstimatedPayment {
    pub paid_on: NaiveDate,
    pub amount: Cents,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Household {
    /// Filing status of the employee's own Form 1040.
    pub filing_status: FilingStatus,
    /// The spouse's Form W-2 box 1 wages, on a joint return only.
    #[serde(default)]
    pub spouse_wages: Cents,
    /// The spouse's Form W-2 box 5 Medicare wages. Required when box 1 wages
    /// are provided: pre-tax retirement deferrals can make the two differ.
    #[serde(default)]
    pub spouse_medicare_wages: Option<Cents>,
    /// Whether the spouse is an active participant in a workplace plan.
    #[serde(default)]
    pub spouse_covered_by_plan: bool,
    /// Age the spouse attains this year, for the IRA catch-up.
    #[serde(default)]
    pub spouse_age_this_year: Option<u32>,
    /// Ordinary income that is neither wages nor investment income. May be
    /// negative, for a business loss.
    #[serde(default)]
    pub other_income: Cents,
    #[serde(default)]
    pub interest_and_ordinary_dividends: Cents,
    #[serde(default)]
    pub qualified_dividends: Cents,
    /// Net short-term gain, or loss when negative.
    #[serde(default)]
    pub short_term_capital_gains: Cents,
    /// Net long-term gain, or loss when negative.
    #[serde(default)]
    pub long_term_capital_gains: Cents,
    /// Adjustments to income, Schedule 1 Part II.
    #[serde(default)]
    pub adjustments: Cents,
    /// Amounts deducted in `adjustments` that IRA modified AGI adds back
    /// (including a traditional IRA deduction). See the IRA worksheets.
    #[serde(default)]
    pub ira_magi_addbacks: Cents,
    /// Used when larger than the standard deduction.
    #[serde(default)]
    pub itemized_deductions: Cents,
    /// Boxes checked for age 65 or blindness, filer and spouse together.
    #[serde(default)]
    pub aged_or_blind_boxes: u32,
    /// Nonrefundable credits, such as the child tax credit.
    #[serde(default)]
    pub credits: Cents,
    /// Federal income tax withheld outside this payroll: a spouse's employer,
    /// a broker's backup withholding.
    #[serde(default)]
    pub other_withholding: Cents,
    /// Total tax on last year's return (Form 1040 line 24).
    #[serde(default)]
    pub prior_year_tax: Option<Cents>,
    /// Adjusted gross income on last year's return (Form 1040 line 11).
    #[serde(default)]
    pub prior_year_agi: Option<Cents>,
    /// Estimated tax payments made for this year.
    #[serde(default, rename = "estimated_payment")]
    pub estimated_payments: Vec<EstimatedPayment>,
}

impl Household {
    pub(super) fn validate(&self) -> Result<()> {
        let joint = self.filing_status == FilingStatus::MarriedFilingJointly;
        if !joint && self.spouse_wages != Cents::ZERO {
            bail!(
                "household.spouse_wages is {}, but a spouse's wages are on the return only when \
                 filing jointly and filing_status is {}. Set spouse_wages to \"0.00\" or file \
                 jointly.",
                self.spouse_wages,
                self.filing_status
            );
        }
        if !joint && self.spouse_medicare_wages.unwrap_or_default() != Cents::ZERO {
            bail!(
                "household.spouse_medicare_wages is set, but a spouse's Medicare wages belong \
                 only on a joint return. Set it to \"0.00\" or file jointly."
            );
        }
        if self.spouse_wages.is_positive() && self.spouse_medicare_wages.is_none() {
            bail!(
                "household.spouse_medicare_wages is required when spouse_wages is positive; \
                 enter the spouse's Form W-2 box 5 wages, which can differ from box 1."
            );
        }
        let max_boxes = if joint {
            2 * AGED_OR_BLIND_BOXES_PER_PERSON
        } else {
            AGED_OR_BLIND_BOXES_PER_PERSON
        };
        if self.aged_or_blind_boxes > max_boxes {
            bail!(
                "household.aged_or_blind_boxes is {}, but a {} return has at most {max_boxes}: \
                 one box for age 65 and one for blindness per person on the return.",
                self.aged_or_blind_boxes,
                self.filing_status
            );
        }
        for (name, value) in [
            ("spouse_wages", self.spouse_wages),
            ("spouse_medicare_wages", self.spouse_medicare_wages.unwrap_or_default()),
            ("interest_and_ordinary_dividends", self.interest_and_ordinary_dividends),
            ("qualified_dividends", self.qualified_dividends),
            ("adjustments", self.adjustments),
            ("ira_magi_addbacks", self.ira_magi_addbacks),
            ("itemized_deductions", self.itemized_deductions),
            ("credits", self.credits),
            ("other_withholding", self.other_withholding),
            ("prior_year_tax", self.prior_year_tax.unwrap_or_default()),
        ] {
            if value < Cents::ZERO {
                bail!("household.{name} is {value}; it cannot be negative");
            }
        }
        if self.ira_magi_addbacks > self.adjustments {
            bail!(
                "household.ira_magi_addbacks is {}, greater than adjustments {}. \
                 Enter only the part of adjustments that IRA modified AGI adds back.",
                self.ira_magi_addbacks,
                self.adjustments
            );
        }
        if self.prior_year_tax.is_some() != self.prior_year_agi.is_some() {
            bail!(
                "household.prior_year_tax and household.prior_year_agi go together: the tax sets \
                 the safe harbor and the adjusted gross income decides whether it is 100% or \
                 110% of it. Set both from last year's Form 1040, lines 24 and 11, or neither."
            );
        }
        for p in &self.estimated_payments {
            if !p.amount.is_positive() {
                bail!(
                    "household.estimated_payment on {} is {}; a payment must be positive",
                    p.paid_on,
                    p.amount
                );
            }
        }
        Ok(())
    }
}
