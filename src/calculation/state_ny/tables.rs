//! New York, New York City and Yonkers tax tables and their loader.

use crate::money::Cents;
use crate::tables::folder;
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

// ---------------------------------------------------------------------------
// Table structures
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct ByStatus {
    pub single: Cents,
    pub married: Cents,
}

/// New York recognises two withholding statuses. A head-of-household filer uses
/// the single tables; there is no third schedule.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NyStatus {
    Single,
    Married,
}

impl NyStatus {
    pub(super) fn pick(self, v: ByStatus) -> Cents {
        match self {
            NyStatus::Single => v.single,
            NyStatus::Married => v.married,
        }
    }
}

#[derive(Clone, Debug)]
pub struct AnnualSchedule {
    pub(super) rows: Vec<(Cents, Cents, i64)>,
}

impl<'de> Deserialize<'de> for AnnualSchedule {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<AnnualSchedule, D::Error> {
        Ok(AnnualSchedule {
            rows: Vec::deserialize(d)?,
        })
    }
}

/// Method III rows carry a threshold and a rate only. The rate multiplies the
/// entire annualized wage: there is no base amount and no excess-over-threshold
/// term, which is why these rows are shaped differently from Method II's and
/// must never be fed to the Method II evaluator.
#[derive(Clone, Debug, Deserialize)]
pub struct TopRates {
    #[serde(flatten)]
    _unused: HashMap<String, toml::Value>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct TopRateSchedules {
    pub single: Vec<(Cents, i64)>,
    pub married: Vec<(Cents, i64)>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct BracketSchedules {
    pub single: AnnualSchedule,
    pub married: AnnualSchedule,
}

impl BracketSchedules {
    pub(super) fn pick(&self, status: NyStatus) -> &AnnualSchedule {
        match status {
            NyStatus::Single => &self.single,
            NyStatus::Married => &self.married,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Nys {
    pub supplemental_rate_ppm: i64,
    pub method_iii_threshold_single: Cents,
    pub method_iii_threshold_married: Cents,
    pub deduction_allowance: HashMap<String, ByStatus>,
    pub exemption_allowance: HashMap<String, Cents>,
    pub period_threshold_quantum: HashMap<String, Cents>,
    pub brackets: BracketSchedules,
    pub top_rates: TopRateSchedules,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PeriodOverride {
    pub period: String,
    /// One-based row index into the derived per-period schedule.
    pub row: usize,
    #[serde(default)]
    pub at_least: Option<Cents>,
    #[serde(default)]
    pub base_tax: Option<Cents>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Nyc {
    pub supplemental_rate_ppm: i64,
    pub deduction_allowance: HashMap<String, ByStatus>,
    pub exemption_allowance: HashMap<String, Cents>,
    pub period_threshold_quantum: HashMap<String, Cents>,
    pub brackets: BracketSchedules,
    #[serde(default)]
    pub period_table_overrides: Vec<PeriodOverride>,
}

/// A Yonkers nonresident exclusion row. The first row carries `false` rather
/// than a zero exclusion, because a zero exclusion would tax the whole
/// annualized wage, which is the opposite of what that row prescribes.
#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum Exclusion {
    None(bool),
    Amount(Cents),
}

#[derive(Clone, Debug, Deserialize)]
pub struct NonresidentExclusions {
    pub rows: Vec<(Cents, Exclusion)>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Yonkers {
    pub resident_surcharge_rate_ppm: i64,
    pub nonresident_earnings_rate_ppm: i64,
    pub resident_supplemental_rate_ppb: i64,
    pub nonresident_supplemental_rate_ppm: i64,
    pub nonresident_exclusions: NonresidentExclusions,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Sdi {
    pub employee_rate_ppm: i64,
    pub employee_weekly_cap: Cents,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Pfl {
    pub employee_rate_ppm: i64,
    pub employee_annual_cap: Cents,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Sui {
    pub wage_base: Cents,
    pub new_employer_total_rate_ppm: i64,
    pub assigned_rate_min_ppm: i64,
    pub assigned_rate_max_ppm: i64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Tables {
    pub year: i32,
    pub source: String,
    pub payroll_periods: HashMap<String, i64>,
    pub nys: Nys,
    pub nyc: Nyc,
    pub yonkers: Yonkers,
    pub sdi: Sdi,
    pub pfl: Pfl,
    pub sui: Sui,
}

impl Tables {
    pub fn load(path: &Path, year: i32) -> Result<Tables> {
        let document = folder::read_folder(path, &|| {
            format!(
                "no New York tax table for {year} at {}. Transcribe that year's NYS-50-T-NYS, \
                 NYS-50-T-NYC and NYS-50-T-Y into section files in this folder.",
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
        Ok(t)
    }
}
