//! The payroll ledger: every run that has been committed, and the year-to-date
//! totals derived from them.
//!
//! The ledger is the source of truth for wage bases, deferral limits, deposit
//! liabilities and every form line value, so it is stored as plain TOML that
//! diffs cleanly and lives in version control beside the configuration. A
//! payroll you cannot reconstruct from a commit is a payroll you cannot defend
//! in an examination.

use std::path::Path;

use anyhow::{bail, Context, Result};
use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};

use crate::calendar::MONTHS_PER_QUARTER;

mod run;
mod totals;

pub use run::PayrollRun;
pub use totals::YearToDate;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Ledger {
    #[serde(default, rename = "run")]
    pub runs: Vec<PayrollRun>,
}

impl Ledger {
    pub fn load(path: &Path) -> Result<Ledger> {
        let text = std::fs::read_to_string(path).with_context(|| {
            format!(
                "no payroll ledger at {}. Run `rachuba init` to write an empty one, or name the \
                 committed ledger with --ledger.",
                path.display()
            )
        })?;
        let ledger: Ledger =
            toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
        ledger.validate()?;
        Ok(ledger)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let header = "# rachuba payroll ledger.\n\
                      #\n\
                      # Every committed payroll run, in pay-date order. This file is the source\n\
                      # of truth for wage bases, deferral limits, deposit liabilities and form\n\
                      # line values. Commit it; never edit an entry that has already been paid.\n\n";
        let body = toml::to_string_pretty(self).context("serializing ledger")?;
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("creating {}", parent.display()))?;
            }
        }
        std::fs::write(path, format!("{header}{body}"))
            .with_context(|| format!("writing {}", path.display()))
    }

    fn validate(&self) -> Result<()> {
        for pair in self.runs.windows(2) {
            if pair[1].pay_date < pair[0].pay_date {
                bail!(
                    "ledger is out of order: {} precedes {}. Year-to-date totals depend on \
                     pay-date order, so entries must be appended chronologically.",
                    pair[1].pay_date,
                    pair[0].pay_date
                );
            }
        }
        Ok(())
    }

    /// Append a run, refusing anything that would corrupt the year-to-date
    /// arithmetic already applied to earlier paychecks.
    pub fn append(&mut self, run: PayrollRun) -> Result<()> {
        if let Some(last) = self.runs.last() {
            if run.pay_date < last.pay_date {
                bail!(
                    "pay date {} is before the last recorded run on {}. A backdated run would \
                     invalidate the wage bases already applied; record a correction dated today \
                     instead.",
                    run.pay_date,
                    last.pay_date
                );
            }
        }
        self.runs.push(run);
        Ok(())
    }

    /// Take back the run recorded for `pay_date`, the inverse of `append`.
    /// Only the last run can go: every later run was computed against its
    /// year-to-date totals. A run whose taxes were deposited or whose
    /// deferral was remitted moved money, so it is refused until those
    /// records are retracted.
    pub fn void(&mut self, pay_date: NaiveDate) -> Result<PayrollRun> {
        let Some(last) = self.runs.last() else {
            bail!("the ledger records no run");
        };
        if last.pay_date != pay_date {
            if self.runs.iter().any(|r| r.pay_date == pay_date) {
                bail!(
                    "the run on {pay_date} is not the last one ({} is); later runs were computed \
                     against its year-to-date totals, so record a correction instead",
                    last.pay_date
                );
            }
            bail!("no recorded run with pay date {pay_date}");
        }
        if let Some(on) = last.taxes_deposited {
            bail!("the run on {pay_date} has its taxes deposited on {on}; retract the deposit first");
        }
        if let Some(on) = last.deferral_remitted {
            bail!("the run on {pay_date} has its deferral remitted on {on}; retract the remittance first");
        }
        Ok(self.runs.pop().expect("the last run was just read"))
    }

    pub fn runs_in_year(&self, year: i32) -> impl Iterator<Item = &PayrollRun> {
        self.runs.iter().filter(move |r| r.pay_date.year() == year)
    }

    pub fn runs_in_quarter(&self, year: i32, quarter: u32) -> impl Iterator<Item = &PayrollRun> {
        self.runs_in_year(year)
            .filter(move |r| (r.pay_date.month() - 1) / MONTHS_PER_QUARTER + 1 == quarter)
    }

    /// Totals for the whole of a calendar year.
    pub fn ytd(&self, year: i32) -> YearToDate {
        let mut acc = YearToDate::default();
        for r in self.runs_in_year(year) {
            acc.accumulate(r);
        }
        acc
    }

    /// Totals for a calendar year strictly before `pay_date`, which is the
    /// context a new run is computed against.
    pub fn ytd_before(&self, pay_date: NaiveDate) -> YearToDate {
        let mut acc = YearToDate::default();
        for r in self.runs_in_year(pay_date.year()) {
            if r.pay_date < pay_date {
                acc.accumulate(r);
            }
        }
        acc
    }

    /// Totals for one quarter, which is what Form 941 reports.
    pub fn quarter_totals(&self, year: i32, quarter: u32) -> YearToDate {
        let mut acc = YearToDate::default();
        for r in self.runs_in_quarter(year, quarter) {
            acc.accumulate(r);
        }
        acc
    }
}




