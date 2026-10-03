//! Employee identity and federal and state withholding certificates.

use super::PayFrequency;
use crate::money::Cents;
use crate::tables::FilingStatus;
use serde::{Deserialize, Serialize};

/// The entries an employer actually needs from Form W-4. Steps 3, 4(a) and
/// 4(b) are annual amounts; Step 4(c) is per pay period.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct W4 {
    pub filing_status: FilingStatus,
    /// Step 2 checkbox: multiple jobs, or a working spouse.
    #[serde(default)]
    pub step2_checkbox: bool,
    /// Step 3, annual: credits for dependents and any other credits claimed.
    #[serde(default)]
    pub step3_credits: Cents,
    /// Step 4(a), annual: other income not from jobs.
    #[serde(default)]
    pub step4a_other_income: Cents,
    /// Step 4(b), annual: deductions beyond the standard deduction.
    #[serde(default)]
    pub step4b_deductions: Cents,
    /// Step 4(c), per pay period: extra withholding requested.
    #[serde(default)]
    pub step4c_extra_withholding: Cents,
}

impl Default for W4 {
    fn default() -> W4 {
        W4 {
            filing_status: FilingStatus::Single,
            step2_checkbox: false,
            step3_credits: Cents::ZERO,
            step4a_other_income: Cents::ZERO,
            step4b_deductions: Cents::ZERO,
            step4c_extra_withholding: Cents::ZERO,
        }
    }
}

/// The withholding status a state recognises, which is not always the federal
/// one. California, for instance, treats "married" as married with a single
/// employer and a non-working spouse; a married employee whose spouse also
/// works belongs in `Single`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StateFilingStatus {
    Single,
    Married,
    HeadOfHousehold,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Employee {
    pub name: String,
    /// Last four digits only. The full number never enters this repository.
    pub ssn_last4: String,
    pub address: String,
    pub pay_frequency: PayFrequency,
    /// Gross wages per pay period.
    pub gross_per_period: Cents,
    /// Age attained during the calendar year, which is what the catch-up rules
    /// key on. Omit it and no catch-up is allowed.
    #[serde(default)]
    pub age_this_year: Option<u32>,
    pub w4: W4,
    /// Free-form state residency flags consumed by the state engine, for
    /// example `nyc_resident` or `yonkers_resident`.
    #[serde(default)]
    pub state_flags: Vec<String>,
    /// Regular withholding allowances on the state certificate: Form DE 4 line
    /// 1 in California, Form IT-2104 in New York.
    #[serde(default)]
    pub state_allowances: i64,
    /// Additional allowances claimed for estimated deductions, Form DE 4 line
    /// 2. A separate quantity from `state_allowances` that must never be added
    /// to it; California reads the two in different tables.
    #[serde(default)]
    pub state_estimated_allowances: i64,
    /// Flat extra state withholding requested per pay period on the state
    /// certificate.
    #[serde(default)]
    pub state_additional_withholding: Cents,
    /// Override the state withholding status. Left unset, it is derived from
    /// the federal filing status, which is right for the common cases and
    /// wrong for a married employee whose spouse also works.
    #[serde(default, rename = "state_filing_status")]
    pub state_filing_status_override: Option<StateFilingStatus>,
}

impl Employee {
    pub fn has_flag(&self, flag: &str) -> bool {
        self.state_flags.iter().any(|f| f == flag)
    }

    /// The state withholding status, derived from the federal one unless
    /// overridden.
    pub fn state_filing_status(&self) -> StateFilingStatus {
        if let Some(s) = self.state_filing_status_override {
            return s;
        }
        match self.w4.filing_status {
            FilingStatus::MarriedFilingJointly => StateFilingStatus::Married,
            FilingStatus::HeadOfHousehold => StateFilingStatus::HeadOfHousehold,
            _ => StateFilingStatus::Single,
        }
    }
}
