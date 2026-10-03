//! A recorded paycheck and its unchanged liability calculations.

use crate::money::Cents;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// One committed payroll run.
///
/// Every figure the run produced is stored, rather than recomputed on demand,
/// because the tables that produced them change every January and a prior
/// year's paycheck must never move.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PayrollRun {
    pub pay_date: NaiveDate,
    /// Free-form label: "regular", "bonus", "correction".
    #[serde(default = "default_kind")]
    pub kind: String,

    pub gross: Cents,
    pub pretax_401k: Cents,
    pub roth_401k: Cents,
    pub employer_401k: Cents,

    /// Wages subject to federal income tax withholding.
    pub fit_wages: Cents,
    /// Wages subject to Social Security and Medicare.
    pub fica_wages: Cents,
    /// The part of `fica_wages` that actually attracted Social Security tax.
    pub ss_taxable_wages: Cents,
    pub futa_taxable_wages: Cents,

    pub federal_income_tax: Cents,
    pub ss_employee: Cents,
    pub ss_employer: Cents,
    pub medicare_employee: Cents,
    pub medicare_employer: Cents,
    pub additional_medicare: Cents,
    pub futa_employer: Cents,

    pub state_income_tax: Cents,
    pub local_income_tax: Cents,
    /// State disability insurance withheld. Separate from paid family leave
    /// because the two carry different caps: disability is capped per week,
    /// paid family leave per year.
    pub sdi_employee: Cents,
    pub pfl_employee: Cents,
    pub sui_employer: Cents,
    /// Employer-side state levy reported separately from unemployment, such as
    /// California's Employment Training Tax.
    #[serde(default)]
    pub state_training_tax: Cents,

    pub net_pay: Cents,

    /// Date the elective deferral was actually moved to the plan trustee. The
    /// Department of Labor safe harbor is seven business days from the pay
    /// date; until this is filled in, the money is still the employer's and
    /// the clock is running.
    #[serde(default)]
    pub deferral_remitted: Option<NaiveDate>,
    /// Date the Form 941 taxes for this run were deposited through EFTPS.
    #[serde(default)]
    pub taxes_deposited: Option<NaiveDate>,
}

fn default_kind() -> String {
    "regular".to_string()
}

impl PayrollRun {
    /// Withheld income tax plus both halves of FICA: the amount that must reach
    /// EFTPS against Form 941.
    pub fn form_941_liability(&self) -> Cents {
        self.federal_income_tax
            + self.ss_employee
            + self.ss_employer
            + self.medicare_employee
            + self.medicare_employer
            + self.additional_medicare
    }

    /// Total elective deferral, both tax treatments, which is what section
    /// 402(g) limits.
    pub fn elective_deferral(&self) -> Cents {
        self.pretax_401k + self.roth_401k
    }

    /// Employee-side state insurance withholding, the two caps summed for
    /// presentation on a stub or a W-2 box 14.
    pub fn state_employee_insurance(&self) -> Cents {
        self.sdi_employee + self.pfl_employee
    }

    /// Everything the company pays out for this run beyond the net paycheck.
    pub fn employer_cost(&self) -> Cents {
        self.gross
            + self.ss_employer
            + self.medicare_employer
            + self.futa_employer
            + self.sui_employer
            + self.state_training_tax
            + self.employer_401k
    }
}
