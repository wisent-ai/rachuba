//! Filing statuses and validated percentage-method withholding schedules.

use crate::money::{Cents, WHOLE_PPM};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Filing status from Step 1(c) of Form W-4.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilingStatus {
    Single,
    MarriedFilingJointly,
    MarriedFilingSeparately,
    HeadOfHousehold,
}

impl FilingStatus {
    /// Publication 15-T publishes three schedules, not four: single and
    /// married-filing-separately share one.
    fn schedule_key(self) -> ScheduleKey {
        match self {
            FilingStatus::Single | FilingStatus::MarriedFilingSeparately => {
                ScheduleKey::SingleOrSeparate
            }
            FilingStatus::MarriedFilingJointly => ScheduleKey::Joint,
            FilingStatus::HeadOfHousehold => ScheduleKey::HeadOfHousehold,
        }
    }

    /// Whether the larger Worksheet 1A line 1g deduction applies.
    pub fn is_joint(self) -> bool {
        matches!(self, FilingStatus::MarriedFilingJointly)
    }
}

impl fmt::Display for FilingStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            FilingStatus::Single => "Single",
            FilingStatus::MarriedFilingJointly => "Married filing jointly",
            FilingStatus::MarriedFilingSeparately => "Married filing separately",
            FilingStatus::HeadOfHousehold => "Head of household",
        };
        f.write_str(s)
    }
}

enum ScheduleKey {
    Joint,
    SingleOrSeparate,
    HeadOfHousehold,
}

/// One row of a percentage-method rate schedule.
///
/// Columns A, C and D of the Publication 15-T tables. Column B, the upper
/// bound, is implied by the next row and is therefore not stored: keeping it
/// would create a second place for a transcription error to hide.
#[derive(Clone, Copy, Debug)]
pub struct Bracket {
    pub at_least: Cents,
    pub base_tax: Cents,
    pub rate_ppm: i64,
}

/// An annual percentage-method rate schedule for one filing status.
#[derive(Clone, Debug)]
pub struct Schedule {
    brackets: Vec<Bracket>,
}

impl Schedule {
    /// Worksheet 1A lines 2b through 2g: find the bracket, take the base
    /// amount, and add the marginal rate applied to the excess.
    pub fn tax_on(&self, amount: Cents) -> Cents {
        let b = self.bracket_for(amount);
        b.base_tax + (amount - b.at_least).mul_ppm(b.rate_ppm)
    }

    pub fn bracket_for(&self, amount: Cents) -> Bracket {
        // Schedules are short (eight rows). A linear scan from the top is
        // clearer than a binary search and costs nothing measurable.
        *self
            .brackets
            .iter()
            .rev()
            .find(|b| amount >= b.at_least)
            .unwrap_or(&self.brackets[0])
    }

    pub(crate) fn validate(&self, what: &str) -> Result<()> {
        if self.brackets.is_empty() {
            bail!("{what}: schedule has no brackets");
        }
        if self.brackets[0].at_least != Cents::ZERO {
            bail!(
                "{what}: first bracket starts at {}, must start at 0.00",
                self.brackets[0].at_least
            );
        }
        for pair in self.brackets.windows(2) {
            if pair[1].at_least <= pair[0].at_least {
                bail!(
                    "{what}: brackets out of order at {} then {}",
                    pair[0].at_least,
                    pair[1].at_least
                );
            }
        }
        for b in &self.brackets {
            if !(0..=WHOLE_PPM).contains(&b.rate_ppm) {
                bail!(
                    "{what}: rate {} ppm at bracket {} is not between 0% and 100%",
                    b.rate_ppm,
                    b.at_least
                );
            }
        }
        Ok(())
    }
}

impl<'de> Deserialize<'de> for Schedule {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Schedule, D::Error> {
        let rows: Vec<(Cents, Cents, i64)> = Vec::deserialize(d)?;
        Ok(Schedule {
            brackets: rows
                .into_iter()
                .map(|(at_least, base_tax, rate_ppm)| Bracket {
                    at_least,
                    base_tax,
                    rate_ppm,
                })
                .collect(),
        })
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct StatusSchedules {
    pub married_filing_jointly: Schedule,
    pub single_or_married_filing_separately: Schedule,
    pub head_of_household: Schedule,
}

impl StatusSchedules {
    fn get(&self, status: FilingStatus) -> &Schedule {
        match status.schedule_key() {
            ScheduleKey::Joint => &self.married_filing_jointly,
            ScheduleKey::SingleOrSeparate => &self.single_or_married_filing_separately,
            ScheduleKey::HeadOfHousehold => &self.head_of_household,
        }
    }

    pub(super) fn validate(&self, what: &str) -> Result<()> {
        self.married_filing_jointly
            .validate(&format!("{what}.married_filing_jointly"))?;
        self.single_or_married_filing_separately
            .validate(&format!("{what}.single_or_married_filing_separately"))?;
        self.head_of_household
            .validate(&format!("{what}.head_of_household"))
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Withholding {
    pub standard_deduction_mfj: Cents,
    pub standard_deduction_other: Cents,
    pub allowance_value: Cents,
    pub standard: StatusSchedules,
    pub step2_checked: StatusSchedules,
}

impl Withholding {
    /// Worksheet 1A step 2: pick the schedule named by the Step 2 checkbox and
    /// the employee's filing status.
    pub fn schedule(&self, status: FilingStatus, step2_checked: bool) -> &Schedule {
        if step2_checked {
            self.step2_checked.get(status)
        } else {
            self.standard.get(status)
        }
    }

    /// Worksheet 1A line 1g.
    pub fn line_1g_deduction(&self, status: FilingStatus, step2_checked: bool) -> Cents {
        if step2_checked {
            Cents::ZERO
        } else if status.is_joint() {
            self.standard_deduction_mfj
        } else {
            self.standard_deduction_other
        }
    }
}




