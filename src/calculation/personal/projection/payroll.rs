//! Completing the ledger's year with the pay periods still to come: every
//! remaining regular period is computed by the engine that computes a real
//! run, dated on the last days of the year, and never recorded.

use anyhow::{bail, Result};
use chrono::{Datelike, Days};

use super::ProjectionRequest;
use crate::ledger::{Ledger, YearToDate};
use crate::run::{self, RunRequest};

/// The label of a run paid on the regular schedule.
const REGULAR_RUN: &str = "regular";

/// The year as the ledger recorded it and as the projected runs complete it.
pub(super) struct CompletedYear {
    /// The ledger up to the end of the year, with the projected runs appended.
    pub ledger: Ledger,
    /// The year-to-date totals of the recorded runs alone.
    pub recorded_totals: YearToDate,
    pub recorded: u32,
    pub remaining: u32,
}

/// The remaining count is the periods in a year less the regular runs
/// already recorded; a ledger whose last run leaves no days for them is
/// refused by name.
pub(super) fn complete_year(req: &ProjectionRequest<'_>) -> Result<CompletedYear> {
    let cfg = req.config;
    let year = req.year;
    let mut ledger = Ledger {
        runs: req
            .ledger
            .runs
            .iter()
            .filter(|r| r.pay_date.year() <= year)
            .cloned()
            .collect(),
    };
    let periods = u32::try_from(cfg.employee.pay_frequency.periods_per_year())?;
    let recorded = ledger
        .runs_in_year(year)
        .filter(|r| r.kind == REGULAR_RUN)
        .count();
    let recorded = u32::try_from(recorded)?;
    let remaining = periods.saturating_sub(recorded);
    let recorded_totals = ledger.ytd(year);
    // The year ends where its last quarter does; the remaining periods are
    // placed on its last days, the latest on December 31.
    let year_end = crate::calendar::quarter_end(year, crate::calendar::QUARTERS_PER_YEAR);
    let before = year_end - Days::new(u64::from(remaining));
    let Some(first) = before.succ_opt() else {
        bail!("{before} has no next day to place the remaining {year} pay periods from");
    };
    for pay_date in first.iter_days().take(usize::try_from(remaining)?) {
        if let Some(last) = ledger.runs.last() {
            if pay_date < last.pay_date {
                bail!(
                    "{remaining} {} pay periods remain in {year} by count, but the ledger's last \
                     run is on {}, leaving no room before December 31 to place them. Record the \
                     runs that were paid, or label off-cycle runs with --kind so they are not \
                     counted as regular.",
                    cfg.employee.pay_frequency.label(),
                    last.pay_date
                );
            }
        }
        let computed = run::compute(&RunRequest {
            config: cfg,
            federal_tables: req.federal,
            state_tables: req.state,
            ledger: &ledger,
            pay_date,
            gross_override: None,
            kind: "projected".to_string(),
        })?;
        ledger.append(computed.run)?;
    }
    Ok(CompletedYear {
        ledger,
        recorded_totals,
        recorded,
        remaining,
    })
}
