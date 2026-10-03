//! Company, employee, and election configuration.
//!
//! One TOML file describes the payer, the one employee, that employee's Form
//! W-4, and their retirement election. It is checked into the company's private
//! repository, so it deliberately holds no Social Security number: the last
//! four digits are enough for a pay stub, and the full number is typed straight
//! into SSA Business Services Online when the W-2 is filed.

use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::calendar::DepositSchedule;
use crate::money::{Cents, WHOLE_PPM};
mod elections;
mod employee;
mod household;

/// An EIN is written `12-3456789`: ten characters with the hyphen third; a stub shows the last four SSN digits.
const EIN_LENGTH: usize = 10;
const EIN_HYPHEN_INDEX: usize = 2;
const SSN_LAST_DIGITS: usize = 4;

pub use elections::{Election, PayFrequency, Retirement401k};
pub use employee::{Employee, StateFilingStatus, W4};
pub use household::{EstimatedPayment, Household};

/// Federal employment tax deposit schedule, determined by the lookback period.
/// A new employer whose lookback-period liability was under $50,000 is a
/// monthly schedule depositor.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DepositScheduleConfig {
    Monthly,
    Semiweekly,
}

impl From<DepositScheduleConfig> for DepositSchedule {
    fn from(d: DepositScheduleConfig) -> DepositSchedule {
        match d {
            DepositScheduleConfig::Monthly => DepositSchedule::Monthly,
            DepositScheduleConfig::Semiweekly => DepositSchedule::Semiweekly,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Company {
    pub legal_name: String,
    /// Employer Identification Number, formatted `12-3456789`.
    pub ein: String,
    pub address: String,
    /// Two-letter postal code. Selects the state table file.
    pub state: String,
    pub deposit_schedule: DepositScheduleConfig,
    /// State unemployment rate from the employer's own annual rate notice, in
    /// parts-per-million. Every employer is assigned an individual rate, so
    /// there is no sane default and the field is required.
    pub sui_rate_ppm: i64,
    /// FUTA credit reduction for states that have not repaid their federal
    /// unemployment loans, in parts-per-million. Zero for most states in most
    /// years; check the annual Schedule A (Form 940) list.
    #[serde(default)]
    pub futa_credit_reduction_ppm: i64,
    /// Round withheld federal income tax to a whole dollar. Publication 15-T
    /// permits it; commercial providers do not do it, and neither does this
    /// engine unless asked.
    #[serde(default)]
    pub round_withholding_to_dollar: bool,
    /// Use the annualized state withholding method where the state publishes
    /// one. California's DE 44 page 45 sanctions it alongside the direct
    /// per-period path, and the two differ by a few cents.
    ///
    /// Defaults to true: twelve annualized payrolls sum to exactly the annual
    /// liability, and it is what commercial providers compute, so adopting this
    /// engine mid-year leaves no step in the year-to-date figures. Hold it
    /// fixed for a whole year.
    #[serde(default = "default_true")]
    pub state_annualized_method: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub company: Company,
    pub employee: Employee,
    #[serde(default)]
    pub retirement_401k: Retirement401k,
    /// The employee's own return. Only `rachuba return` reads it.
    #[serde(default)]
    pub household: Option<Household>,
}

impl Config {
    pub fn load(path: &Path) -> Result<Config> {
        let text = std::fs::read_to_string(path).with_context(|| {
            format!(
                "no configuration at {}. Run `rachuba init` to write a starting file.",
                path.display()
            )
        })?;
        let config: Config =
            toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
        config
            .validate()
            .with_context(|| format!("validating {}", path.display()))?;
        Ok(config)
    }

    fn validate(&self) -> Result<()> {
        if self.company.state.len() != 2
            || !self.company.state.chars().all(|c| c.is_ascii_uppercase())
        {
            bail!(
                "company.state must be a two-letter uppercase postal code, got {:?}",
                self.company.state
            );
        }
        let ein = &self.company.ein;
        let ein_shape = ein.len() == EIN_LENGTH
            && ein.as_bytes()[EIN_HYPHEN_INDEX] == b'-'
            && ein.bytes().enumerate().all(|(i, b)| {
                if i == EIN_HYPHEN_INDEX {
                    b == b'-'
                } else {
                    b.is_ascii_digit()
                }
            });
        if !ein_shape {
            bail!("company.ein must look like 12-3456789, got {ein:?}");
        }
        if self.employee.ssn_last4.len() != SSN_LAST_DIGITS
            || !self.employee.ssn_last4.chars().all(|c| c.is_ascii_digit())
        {
            bail!("employee.ssn_last4 must be exactly four digits");
        }
        if self.employee.gross_per_period <= Cents::ZERO {
            bail!("employee.gross_per_period must be positive");
        }
        if self.company.sui_rate_ppm < 0 {
            bail!("company.sui_rate_ppm cannot be negative");
        }
        for field in [
            ("retirement_401k.pretax", self.retirement_401k.pretax),
            ("retirement_401k.roth", self.retirement_401k.roth),
            ("retirement_401k.employer", self.retirement_401k.employer),
        ] {
            if let Election::Percent { ppm } = field.1 {
                if !(0..=WHOLE_PPM).contains(&ppm) {
                    bail!("{}: {} ppm is not between 0% and 100%", field.0, ppm);
                }
            }
            if let Election::PerPeriod { amount } = field.1 {
                if amount < Cents::ZERO {
                    bail!("{}: amount cannot be negative", field.0);
                }
            }
        }
        if let Some(h) = &self.household {
            h.validate()?;
        }
        Ok(())
    }

    /// The starting file written by `rachuba init`, with every field present so
    /// the operator edits rather than guesses.
    pub fn template() -> &'static str {
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/rachuba.toml"))
    }
}
