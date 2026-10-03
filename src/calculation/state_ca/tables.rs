//! Published California tables and their load-time validation.

use crate::money::Cents;
use crate::tables::folder;
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// The four printed columns of Tables 1 and 3. There are four columns but only
/// three rate schedules, because the married column splits on allowance count
/// rather than on status.
#[derive(Clone, Copy, Debug, Deserialize)]
pub struct ByColumn {
    pub single_or_dual_income_married: Cents,
    pub married_0_or_1_allowances: Cents,
    pub married_2_or_more_allowances: Cents,
    pub head_of_household: Cents,
}

/// California's three withholding statuses.
///
/// `Married` means married with one employer and a non-working spouse. A
/// married employee whose spouse also works, or who holds a second job, belongs
/// in `Single`; DE 44 recommends it and the engine takes the status as given
/// rather than reclassifying.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CaStatus {
    Single,
    Married,
    HeadOfHousehold,
}

impl CaStatus {
    /// Column selection for Tables 1 and 3. Uses the REGULAR allowance count,
    /// never the estimated one, and never the rate-table status.
    pub(super) fn column(self, regular: i64) -> fn(&ByColumn) -> Cents {
        match self {
            CaStatus::Single => |c: &ByColumn| c.single_or_dual_income_married,
            CaStatus::HeadOfHousehold => |c: &ByColumn| c.head_of_household,
            CaStatus::Married => {
                if regular >= 2 {
                    |c: &ByColumn| c.married_2_or_more_allowances
                } else {
                    |c: &ByColumn| c.married_0_or_1_allowances
                }
            }
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct StatusBrackets {
    pub single_or_dual_income_married: Vec<(Cents, Cents, i64)>,
    pub married_one_employer: Vec<(Cents, Cents, i64)>,
    pub head_of_household: Vec<(Cents, Cents, i64)>,
}

impl StatusBrackets {
    /// Rate-table selection, by marital status alone. No allowance-count split
    /// here: that rule belongs to Tables 1 and 3 only.
    pub(super) fn pick(&self, status: CaStatus) -> &[(Cents, Cents, i64)] {
        match status {
            CaStatus::Single => &self.single_or_dual_income_married,
            CaStatus::Married => &self.married_one_employer,
            CaStatus::HeadOfHousehold => &self.head_of_household,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Pit {
    pub supplemental_rate_bonus_and_stock_options_ppm: i64,
    pub supplemental_rate_other_ppm: i64,
    pub low_income_exemption: HashMap<String, ByColumn>,
    pub standard_deduction: HashMap<String, ByColumn>,
    /// Ten cells per period, indexed by allowance count minus one.
    pub estimated_deduction: HashMap<String, Vec<Cents>>,
    pub exemption_allowance: HashMap<String, Vec<Cents>>,
    pub brackets: HashMap<String, StatusBrackets>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Sdi {
    pub employee_rate_ppm: i64,
    pub statutory_min_rate_ppm: i64,
    pub statutory_max_rate_ppm: i64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Sui {
    pub wage_base: Cents,
    pub new_employer_rate_ppm: i64,
    pub assigned_rate_min_ppm: i64,
    pub assigned_rate_max_ppm: i64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Ett {
    pub rate_ppm: i64,
    pub wage_base: Cents,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Local {
    /// California bars local income taxes outright: Revenue and Taxation Code
    /// section 17041.5. San Francisco's Payroll Expense Tax was repealed
    /// effective 2021 and its Gross Receipts Tax is a business tax, not a
    /// withholding. This is always false and exists so the fact is asserted
    /// rather than assumed.
    pub employee_withholding: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct FutaCreditReduction {
    /// Final rates from Schedule A (Form 940), by tax year. Absence of the
    /// current year means its rate has not been determined.
    pub determined: Vec<(i32, i64)>,
    /// Date on which the current year's advance balance is tested.
    pub test_date: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Tables {
    pub year: i32,
    pub source: String,
    pub payroll_periods: HashMap<String, i64>,
    pub pit: Pit,
    pub sdi: Sdi,
    pub sui: Sui,
    pub ett: Ett,
    pub local: Local,
    pub futa_credit_reduction: FutaCreditReduction,
}

impl Tables {
    pub fn load(path: &Path, year: i32) -> Result<Tables> {
        let document = folder::read_folder(path, &|| {
            format!(
                "no California tax table for {year} at {}. Transcribe that year's DE 44 and \
                 the Method B withholding schedules into section files in this folder.",
                path.display()
            )
        })?;
        let t: Tables = toml::Value::Table(document)
            .try_into()
            .with_context(|| format!("reading {}", path.display()))?;
        if t.year != year {
            bail!(
                "{} declares year {} but was loaded as {year}",
                path.display(),
                t.year
            );
        }
        if !(t.sdi.statutory_min_rate_ppm..=t.sdi.statutory_max_rate_ppm)
            .contains(&t.sdi.employee_rate_ppm)
        {
            bail!(
                "sdi.employee_rate_ppm of {} is outside the statutory range {}-{}",
                t.sdi.employee_rate_ppm,
                t.sdi.statutory_min_rate_ppm,
                t.sdi.statutory_max_rate_ppm
            );
        }
        Ok(t)
    }
}
