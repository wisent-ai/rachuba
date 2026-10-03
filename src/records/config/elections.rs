//! Pay frequency and retirement contribution elections.

use crate::money::Cents;
use serde::{Deserialize, Serialize};

/// How often wages are paid. The variants are exactly the payroll periods in
/// Table 3 of Worksheet 1A, because that table is what the annualization step
/// divides by.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PayFrequency {
    Weekly,
    Biweekly,
    Semimonthly,
    Monthly,
    Quarterly,
    Semiannually,
    Annually,
    Daily,
}

impl PayFrequency {
    /// Worksheet 1A line 1b, from Table 3.
    pub fn periods_per_year(self) -> i64 {
        match self {
            PayFrequency::Weekly => 52,
            PayFrequency::Biweekly => 26,
            PayFrequency::Semimonthly => 24,
            PayFrequency::Monthly => 12,
            PayFrequency::Quarterly => 4,
            PayFrequency::Semiannually => 2,
            PayFrequency::Annually => 1,
            PayFrequency::Daily => 260,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            PayFrequency::Weekly => "weekly",
            PayFrequency::Biweekly => "biweekly",
            PayFrequency::Semimonthly => "semimonthly",
            PayFrequency::Monthly => "monthly",
            PayFrequency::Quarterly => "quarterly",
            PayFrequency::Semiannually => "semiannually",
            PayFrequency::Annually => "annually",
            PayFrequency::Daily => "daily",
        }
    }
}

/// An amount elected either as a share of gross wages or as a flat figure per
/// pay period. Percentages are parts-per-million, so 7.5% is 75000.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum Election {
    #[default]
    None,
    Percent {
        ppm: i64,
    },
    PerPeriod {
        amount: Cents,
    },
}

impl Election {
    pub fn applied_to(self, gross: Cents) -> Cents {
        match self {
            Election::None => Cents::ZERO,
            Election::Percent { ppm } => gross.mul_ppm(ppm),
            Election::PerPeriod { amount } => amount,
        }
    }
}

/// Section 401(k) elections. Pre-tax deferrals reduce wages subject to income
/// tax withholding but not wages subject to Social Security and Medicare;
/// designated Roth deferrals reduce neither.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Retirement401k {
    #[serde(default)]
    pub pretax: Election,
    #[serde(default)]
    pub roth: Election,
    /// Employer contribution as a share of this period's gross wages. In a
    /// year with a loss the company's deduction for it adds to the net
    /// operating loss carried forward.
    #[serde(default)]
    pub employer: Election,
    /// Prior-year Social Security wages, used to decide whether catch-up
    /// contributions must be designated Roth under SECURE 2.0 section 603.
    #[serde(default)]
    pub prior_year_ss_wages: Cents,
}
