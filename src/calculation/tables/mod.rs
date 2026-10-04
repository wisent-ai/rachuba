//! Loading and validation of the versioned tax-table data files.
//!
//! Tax tables are data, not code. Each calendar year gets one folder of section
//! files under the tables directory, and rolling the engine forward is a transcription job
//! against that year's Publication 15-T rather than a code change. That split
//! is the whole maintenance story for this crate, so the loader is strict: a
//! table that is malformed, out of order, or missing its zero bracket is
//! rejected at load time rather than silently producing a wrong paycheck.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::Deserialize;

use crate::money::Cents;

pub mod folder;
mod schedules;
pub use schedules::{Bracket, FilingStatus, Schedule, StatusSchedules, Withholding};

#[derive(Clone, Debug, Deserialize)]
pub struct SocialSecurity {
    pub employee_rate_ppm: i64,
    pub employer_rate_ppm: i64,
    pub wage_base: Cents,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Medicare {
    pub employee_rate_ppm: i64,
    pub employer_rate_ppm: i64,
    pub additional_rate_ppm: i64,
    pub additional_threshold: Cents,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Futa {
    pub gross_rate_ppm: i64,
    pub state_credit_ppm: i64,
    pub wage_base: Cents,
    pub deposit_threshold: Cents,
}

impl Futa {
    /// The net rate an employer actually pays when its state unemployment
    /// contributions were made on time and the state is not a credit-reduction
    /// state.
    pub fn net_rate_ppm(&self) -> i64 {
        self.gross_rate_ppm - self.state_credit_ppm
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Retirement {
    pub elective_deferral_limit: Cents,
    pub catch_up_50_to_59_or_64_plus: Cents,
    pub catch_up_60_to_63: Cents,
    pub annual_additions_limit: Cents,
    pub compensation_limit: Cents,
    pub roth_catch_up_wage_threshold: Cents,
    /// Section 414(v)(1): the age from which catch-up contributions begin.
    pub catch_up_age: u32,
    /// Section 414(v)(2)(E): the first and last ages of the higher catch-up.
    pub higher_catch_up_first_age: u32,
    pub higher_catch_up_last_age: u32,
    /// Section 404(a)(3)(A): the employer's deductible share of compensation.
    pub employer_deduction_ppm: i64,
}

impl Retirement {
    /// Section 414(v) catch-up available at a given age, using the age the
    /// employee attains during the calendar year and the ages the year's table
    /// declares.
    pub fn catch_up_for_age(&self, age: Option<u32>) -> Cents {
        match age {
            Some(a)
                if (self.higher_catch_up_first_age..=self.higher_catch_up_last_age)
                    .contains(&a) =>
            {
                self.catch_up_60_to_63
            }
            Some(a) if a >= self.catch_up_age => self.catch_up_50_to_59_or_64_plus,
            _ => Cents::ZERO,
        }
    }

    /// The employee's own deferral ceiling for the year, catch-up included.
    pub fn deferral_ceiling(&self, age: Option<u32>) -> Cents {
        self.elective_deferral_limit + self.catch_up_for_age(age)
    }
}

/// Every federal parameter for one calendar year.
#[derive(Clone, Debug, Deserialize)]
pub struct FederalTables {
    pub year: i32,
    pub source: String,
    pub withholding: Withholding,
    pub social_security: SocialSecurity,
    pub medicare: Medicare,
    pub futa: Futa,
    pub retirement: Retirement,
}

impl FederalTables {
    pub fn load(dir: &Path, year: i32) -> Result<FederalTables> {
        let path = dir.join(format!("federal-{year}"));
        let document = folder::read_folder(&path, &|| {
            format!(
                "no federal tax table for {year} at {}. Tax tables are per-year data folders; \
                 transcribe that year's Publication 15-T into section files there before \
                 running payroll.",
                path.display()
            )
        })?;
        let tables: FederalTables = toml::Value::Table(document)
            .try_into()
            .with_context(|| format!("reading {}", path.display()))?;
        tables
            .validate()
            .with_context(|| format!("validating {}", path.display()))?;
        if tables.year != year {
            bail!(
                "{} declares year {} but was loaded as {year}",
                path.display(),
                tables.year
            );
        }
        Ok(tables)
    }

    fn validate(&self) -> Result<()> {
        self.withholding.standard.validate("withholding.standard")?;
        self.withholding
            .step2_checked
            .validate("withholding.step2_checked")?;
        if self.social_security.wage_base <= Cents::ZERO {
            bail!("social_security.wage_base must be positive");
        }
        if self.futa.state_credit_ppm > self.futa.gross_rate_ppm {
            bail!("futa.state_credit_ppm exceeds futa.gross_rate_ppm");
        }
        if self.retirement.elective_deferral_limit <= Cents::ZERO {
            bail!("retirement.elective_deferral_limit must be positive");
        }
        if self.retirement.higher_catch_up_first_age > self.retirement.higher_catch_up_last_age {
            bail!(
                "retirement.higher_catch_up_first_age is after retirement.higher_catch_up_last_age"
            );
        }
        if !(0..=crate::money::WHOLE_PPM).contains(&self.retirement.employer_deduction_ppm) {
            bail!("retirement.employer_deduction_ppm must be a share between 0 and 1,000,000 ppm");
        }
        Ok(())
    }
}

/// The directory holding the per-year table folders: `RACHUBA_TABLES` when it
/// is set, otherwise `tables` beside the configuration file. Either must exist;
/// a set variable naming a missing directory is refused rather than skipped.
pub fn resolve_dir(config_path: &Path) -> Result<PathBuf> {
    if let Some(env) = std::env::var_os("RACHUBA_TABLES") {
        let p = PathBuf::from(env);
        if !p.is_dir() {
            bail!(
                "RACHUBA_TABLES names {}, which is not a directory. Point it at the folder \
                 holding federal-<year> and state-<code>-<year>, or unset it to use the \
                 tables folder beside the configuration file.",
                p.display()
            );
        }
        return Ok(p);
    }
    let parent = config_path
        .parent()
        .with_context(|| format!("{} has no parent directory", config_path.display()))?;
    let p = parent.join("tables");
    if !p.is_dir() {
        bail!(
            "no tables directory at {}. Put the per-year table folders there, or set \
             RACHUBA_TABLES to the directory holding them.",
            p.display()
        );
    }
    Ok(p)
}
