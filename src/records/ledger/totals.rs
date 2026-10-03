//! Year-to-date projections derived from committed payroll runs.

use super::PayrollRun;
use crate::money::Cents;

/// Year-to-date totals, always for a single calendar year.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct YearToDate {
    pub gross: Cents,
    pub fit_wages: Cents,
    pub fica_wages: Cents,
    pub ss_wages: Cents,
    pub futa_wages: Cents,
    pub pretax_401k: Cents,
    pub roth_401k: Cents,
    pub employer_401k: Cents,
    pub federal_income_tax: Cents,
    pub ss_employee: Cents,
    pub ss_employer: Cents,
    pub medicare_employee: Cents,
    pub medicare_employer: Cents,
    pub additional_medicare: Cents,
    pub futa_employer: Cents,
    pub state_income_tax: Cents,
    pub local_income_tax: Cents,
    pub sdi_employee: Cents,
    pub pfl_employee: Cents,
    pub sui_employer: Cents,
    pub state_training_tax: Cents,
    pub net_pay: Cents,
    pub runs: u32,
}

impl YearToDate {
    pub(super) fn accumulate(&mut self, r: &PayrollRun) {
        self.gross += r.gross;
        self.fit_wages += r.fit_wages;
        self.fica_wages += r.fica_wages;
        self.ss_wages += r.ss_taxable_wages;
        self.futa_wages += r.futa_taxable_wages;
        self.pretax_401k += r.pretax_401k;
        self.roth_401k += r.roth_401k;
        self.employer_401k += r.employer_401k;
        self.federal_income_tax += r.federal_income_tax;
        self.ss_employee += r.ss_employee;
        self.ss_employer += r.ss_employer;
        self.medicare_employee += r.medicare_employee;
        self.medicare_employer += r.medicare_employer;
        self.additional_medicare += r.additional_medicare;
        self.futa_employer += r.futa_employer;
        self.state_income_tax += r.state_income_tax;
        self.local_income_tax += r.local_income_tax;
        self.sdi_employee += r.sdi_employee;
        self.pfl_employee += r.pfl_employee;
        self.sui_employer += r.sui_employer;
        self.state_training_tax += r.state_training_tax;
        self.net_pay += r.net_pay;
        self.runs += 1;
    }

    pub fn elective_deferral(&self) -> Cents {
        self.pretax_401k + self.roth_401k
    }

    /// Employee-side state insurance withholding, the two caps summed.
    pub fn state_employee_insurance(&self) -> Cents {
        self.sdi_employee + self.pfl_employee
    }

    /// Everything the company paid out this year beyond the net paychecks.
    pub fn employer_cost(&self) -> Cents {
        self.gross
            + self.ss_employer
            + self.medicare_employer
            + self.futa_employer
            + self.sui_employer
            + self.state_training_tax
            + self.employer_401k
    }

    /// Section 415(c) annual additions: deferrals plus employer contributions.
    pub fn annual_additions(&self) -> Cents {
        self.elective_deferral() + self.employer_401k
    }

    /// Box 1 of Form W-2: wages subject to federal income tax. Pre-tax
    /// deferrals are already excluded; Roth deferrals are not.
    pub fn w2_box1(&self) -> Cents {
        self.fit_wages
    }
}
