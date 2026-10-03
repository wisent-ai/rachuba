//! Public command names and arguments, separate from command effects.

use chrono::NaiveDate;
use clap::{Parser, Subcommand};
use rachuba::money::Cents;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "rachuba", version, about = "Single-employee US payroll")]
pub(super) struct Cli {
    /// Configuration file.
    #[arg(long, short, default_value = "rachuba.toml", global = true)]
    pub(super) config: PathBuf,

    /// Payroll ledger file.
    #[arg(long, short, default_value = "ledger.toml", global = true)]
    pub(super) ledger: PathBuf,

    /// Print the answer as JSON for a machine instead of the text for a person;
    /// both come from the same figures.
    #[arg(long, global = true)]
    pub(super) json: bool,

    #[command(subcommand)]
    pub(super) command: Command,
}

#[derive(Subcommand)]
pub(super) enum Command {
    /// Write a starting configuration file.
    Init,

    /// Compute a payroll run. Records nothing unless --commit is given.
    Run {
        /// Pay date. Defaults to today. The tax year is the year of this date.
        #[arg(long)]
        pay_date: Option<NaiveDate>,
        /// Override the configured gross for this period, for a bonus or a
        /// short period.
        #[arg(long)]
        gross: Option<Cents>,
        /// Label for the run.
        #[arg(long, default_value = "regular")]
        kind: String,
        /// Append the run to the ledger.
        #[arg(long)]
        commit: bool,
    },

    /// Print Worksheet 1A line by line for a prospective run.
    Explain {
        #[arg(long)]
        pay_date: Option<NaiveDate>,
        #[arg(long)]
        gross: Option<Cents>,
    },

    /// Print the pay stub for a recorded run.
    Stub {
        #[arg(long)]
        pay_date: NaiveDate,
    },

    /// Year-to-date totals.
    Ytd {
        #[arg(long)]
        year: Option<i32>,
    },

    /// Money that has to move, and by when.
    Owed {
        #[arg(long)]
        year: Option<i32>,
    },

    /// Record that a run's Form 941 taxes were deposited through EFTPS.
    Deposited {
        #[arg(long)]
        pay_date: NaiveDate,
        /// Date of the deposit. Defaults to today.
        #[arg(long)]
        on: Option<NaiveDate>,
    },

    /// Record that a run's elective deferral reached the plan trustee.
    Remitted {
        #[arg(long)]
        pay_date: NaiveDate,
        #[arg(long)]
        on: Option<NaiveDate>,
    },

    /// Take back the last recorded run, the inverse of `run --commit`.
    /// Refused for an earlier run, and for one whose taxes were deposited or
    /// whose deferral was remitted until `retract` undoes that record.
    Void {
        #[arg(long)]
        pay_date: NaiveDate,
    },

    /// Take back a `deposited` or `remitted` record that was entered by mistake.
    Retract {
        #[arg(long)]
        pay_date: NaiveDate,
        /// Which record to take back.
        #[arg(long, value_enum)]
        record: Record,
    },

    /// Filing and deposit due dates for a year.
    Calendar {
        #[arg(long)]
        year: Option<i32>,
    },

    /// Check the year against the statutory limits.
    Check {
        #[arg(long)]
        year: Option<i32>,
    },

    /// Line values for a federal return.
    Form {
        #[command(subcommand)]
        which: FormKind,
    },

    /// Project the employee's own federal return for a year: tax, estimated
    /// tax installments, 401(k) room and IRA eligibility. Reads the
    /// [household] section of the configuration and records nothing.
    Return {
        #[arg(long)]
        year: Option<i32>,
    },
}

/// The two payment records a run carries: the inverse of `deposited` and of `remitted`.
#[derive(Clone, Copy, clap::ValueEnum)]
pub(super) enum Record {
    /// The EFTPS deposit of the run's Form 941 taxes.
    Deposit,
    /// The elective deferral reaching the plan trustee.
    Remittance,
}

#[derive(Subcommand)]
pub(super) enum FormKind {
    /// Employer's Quarterly Federal Tax Return.
    #[command(name = "941")]
    F941 {
        #[arg(long)]
        year: Option<i32>,
        #[arg(long)]
        quarter: u32,
    },
    /// Employer's Annual Federal Unemployment Tax Return.
    #[command(name = "940")]
    F940 {
        #[arg(long)]
        year: Option<i32>,
    },
    /// Wage and Tax Statement.
    W2 {
        #[arg(long)]
        year: Option<i32>,
    },
}
