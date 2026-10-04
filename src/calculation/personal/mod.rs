//! The employee's own federal income tax return.
//!
//! A payroll run withholds against Form W-4; the return is where the year is
//! settled, and it sees income the payroll never does: a spouse's wages,
//! interest, dividends and capital gains. This module projects that return
//! from the ledger, the remaining pay periods and the `[household]` section
//! of the configuration, so the employee can see the year's tax, what the
//! estimated tax safe harbor still asks for, and what each retirement choice
//! is worth before the year closes.
//!
//! The figures are data in `tables/federal-<year>/form-1040.toml` and
//! `ira.toml`, transcribed from the year's revenue procedure and IRS notices,
//! exactly like the withholding tables.

mod estimated;
mod ira;
mod projection;
mod tax;

pub use estimated::{EstimatedTax, Installment};
pub use ira::IraEligibility;
pub use projection::{project, PayrollYear, Projection, ProjectionRequest, Retirement};
pub use tax::{compute_tax, ReturnIncome, TaxComputation};

use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::Deserialize;

use crate::money::Cents;
use crate::tables::{folder, FilingStatus, Schedule};

/// One value per filing status.
#[derive(Clone, Debug, Deserialize)]
pub struct PerStatus<T> {
    pub single: T,
    pub married_filing_jointly: T,
    pub married_filing_separately: T,
    pub head_of_household: T,
}

impl<T> PerStatus<T> {
    pub fn get(&self, status: FilingStatus) -> &T {
        match status {
            FilingStatus::Single => &self.single,
            FilingStatus::MarriedFilingJointly => &self.married_filing_jointly,
            FilingStatus::MarriedFilingSeparately => &self.married_filing_separately,
            FilingStatus::HeadOfHousehold => &self.head_of_household,
        }
    }
}

/// A value the statute sets for joint returns, separate returns, and every
/// other filer (single and head of household) alike.
#[derive(Clone, Debug, Deserialize)]
pub struct JointSeparateOther<T> {
    pub married_filing_jointly: T,
    pub married_filing_separately: T,
    pub other: T,
}

impl<T> JointSeparateOther<T> {
    pub fn get(&self, status: FilingStatus) -> &T {
        match status {
            FilingStatus::MarriedFilingJointly => &self.married_filing_jointly,
            FilingStatus::MarriedFilingSeparately => &self.married_filing_separately,
            FilingStatus::Single | FilingStatus::HeadOfHousehold => &self.other,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct StandardDeduction {
    #[serde(flatten)]
    pub basic: PerStatus<Cents>,
    pub aged_or_blind_married: Cents,
    pub aged_or_blind_unmarried: Cents,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CapitalGains {
    pub middle_rate_ppm: i64,
    pub top_rate_ppm: i64,
    /// [maximum zero rate amount, maximum 15 percent rate amount].
    #[serde(flatten)]
    pub thresholds: PerStatus<(Cents, Cents)>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CapitalLossLimit {
    pub married_filing_separately: Cents,
    pub other: Cents,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RateAboveThreshold {
    pub rate_ppm: i64,
    #[serde(flatten)]
    pub threshold: JointSeparateOther<Cents>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct EstimatedTaxRules {
    pub current_year_ppm: i64,
    pub prior_year_ppm: i64,
    pub prior_year_high_income_ppm: i64,
    pub high_income_agi: Cents,
    pub high_income_agi_separate: Cents,
    pub minimum_balance_due: Cents,
    /// The required installments' statutory dates, in order, as the year's
    /// table declares them.
    pub installments: Vec<InstallmentDate>,
}

/// One required installment's statutory date: `month` and `day` of the tax
/// year plus `year_offset` years, before any weekend or holiday shift.
#[derive(Clone, Copy, Debug, Deserialize)]
pub struct InstallmentDate {
    pub month: u32,
    pub day: u32,
    pub year_offset: i32,
}

/// The `[individual]` section of a federal tax year's folder.
#[derive(Clone, Debug, Deserialize)]
pub struct IndividualTables {
    pub source: String,
    pub rates: PerStatus<Schedule>,
    pub standard_deduction: StandardDeduction,
    pub capital_gains: CapitalGains,
    pub capital_loss_limit: CapitalLossLimit,
    pub net_investment_income_tax: RateAboveThreshold,
    pub additional_medicare: RateAboveThreshold,
    pub estimated_tax: EstimatedTaxRules,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SpouseCoveredRanges {
    pub married_filing_jointly: (Cents, Cents),
    pub married_filing_separately: (Cents, Cents),
}

/// The `[ira]` section of a federal tax year's folder. Every range is
/// [phase-out begins, phase-out complete] of modified adjusted gross income.
#[derive(Clone, Debug, Deserialize)]
pub struct IraTables {
    pub source: String,
    pub contribution_limit: Cents,
    pub catch_up: Cents,
    pub catch_up_age: u32,
    pub rounding_increment: Cents,
    pub minimum_reduced_limit: Cents,
    pub deduction_covered: JointSeparateOther<(Cents, Cents)>,
    pub deduction_spouse_covered: SpouseCoveredRanges,
    pub roth: JointSeparateOther<(Cents, Cents)>,
}

/// Everything the projection of one year's return reads.
#[derive(Clone, Debug)]
pub struct ReturnTables {
    pub year: i32,
    pub individual: IndividualTables,
    pub ira: IraTables,
}

#[derive(Deserialize)]
struct YearFolder {
    year: i32,
    individual: Option<IndividualTables>,
    ira: Option<IraTables>,
}

impl ReturnTables {
    /// Read the `[individual]` and `[ira]` sections from the same
    /// `federal-<year>` folder the payroll tables come from.
    pub fn load(dir: &Path, year: i32) -> Result<ReturnTables> {
        let path = dir.join(format!("federal-{year}"));
        let document = folder::read_folder(&path, &|| {
            format!(
                "no federal tax table for {year} at {}. Transcribe that year's figures into \
                 section files there before projecting its return.",
                path.display()
            )
        })?;
        let parsed: YearFolder = toml::Value::Table(document)
            .try_into()
            .with_context(|| format!("reading {}", path.display()))?;
        if parsed.year != year {
            bail!(
                "{} declares year {} but was loaded as {year}",
                path.display(),
                parsed.year
            );
        }
        let individual = parsed.individual.with_context(|| {
            format!(
                "{} has no [individual] section, so the {year} return cannot be projected. \
                 Transcribe the rate schedules, standard deduction and capital gains thresholds \
                 from that year's revenue procedure into form-1040.toml there.",
                path.display()
            )
        })?;
        let ira = parsed.ira.with_context(|| {
            format!(
                "{} has no [ira] section, so IRA eligibility for {year} cannot be judged. \
                 Transcribe that year's IRA limit and phase-out ranges into ira.toml there.",
                path.display()
            )
        })?;
        let tables = ReturnTables {
            year,
            individual,
            ira,
        };
        tables
            .validate()
            .with_context(|| format!("validating {}", path.display()))?;
        Ok(tables)
    }

    fn validate(&self) -> Result<()> {
        let installments = &self.individual.estimated_tax.installments;
        if installments.is_empty() {
            bail!("individual.estimated_tax.installments declares no installment");
        }
        for (index, date) in installments.iter().enumerate() {
            if chrono::NaiveDate::from_ymd_opt(self.year + date.year_offset, date.month, date.day)
                .is_none()
            {
                bail!(
                    "individual.estimated_tax.installments[{index}] is not a date: month {}, day {}, year offset {}",
                    date.month,
                    date.day,
                    date.year_offset
                );
            }
        }
        let r = &self.individual.rates;
        r.single.validate("individual.rates.single")?;
        r.married_filing_jointly
            .validate("individual.rates.married_filing_jointly")?;
        r.married_filing_separately
            .validate("individual.rates.married_filing_separately")?;
        r.head_of_household
            .validate("individual.rates.head_of_household")?;
        let g = &self.individual.capital_gains.thresholds;
        for (name, (zero, fifteen)) in [
            ("single", g.single),
            ("married_filing_jointly", g.married_filing_jointly),
            ("married_filing_separately", g.married_filing_separately),
            ("head_of_household", g.head_of_household),
        ] {
            if zero > fifteen {
                bail!("individual.capital_gains.{name}: the zero rate amount {zero} exceeds the 15% amount {fifteen}");
            }
        }
        let i = &self.ira;
        for (name, (low, high)) in [
            (
                "deduction_covered.married_filing_jointly",
                i.deduction_covered.married_filing_jointly,
            ),
            (
                "deduction_covered.married_filing_separately",
                i.deduction_covered.married_filing_separately,
            ),
            ("deduction_covered.other", i.deduction_covered.other),
            (
                "deduction_spouse_covered.married_filing_jointly",
                i.deduction_spouse_covered.married_filing_jointly,
            ),
            (
                "deduction_spouse_covered.married_filing_separately",
                i.deduction_spouse_covered.married_filing_separately,
            ),
            ("roth.married_filing_jointly", i.roth.married_filing_jointly),
            (
                "roth.married_filing_separately",
                i.roth.married_filing_separately,
            ),
            ("roth.other", i.roth.other),
        ] {
            if low >= high {
                bail!(
                    "ira.{name}: the phase-out must begin below where it ends, got {low} to {high}"
                );
            }
        }
        if !i.rounding_increment.is_positive() {
            bail!("ira.rounding_increment must be positive");
        }
        Ok(())
    }
}
