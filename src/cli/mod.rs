//! Command dispatch and shared loading of payroll configuration and state.

mod arguments;
mod obligations;
mod paychecks;
mod reports;

use anyhow::{bail, Context, Result};
use arguments::{Cli, Command};
use chrono::{Datelike, Local, NaiveDate};
use clap::Parser;
use obligations::{cmd_calendar, cmd_check, cmd_mark, cmd_owed, cmd_retract};
use paychecks::{cmd_explain, cmd_run, cmd_stub, cmd_void};
use rachuba::{
    config::Config,
    ledger::Ledger,
    state::StateTables,
    tables::{self, FederalTables},
};
use reports::{cmd_form, cmd_return, cmd_ytd};
use std::path::Path;

pub(super) fn main() -> Result<()> {
    let cli = Cli::parse();
    match &cli.command {
        Command::Init => cmd_init(&cli.config, &cli.ledger, cli.json),
        Command::Run {
            pay_date,
            gross,
            kind,
            commit,
        } => cmd_run(&cli, *pay_date, *gross, kind, *commit),
        Command::Explain { pay_date, gross } => cmd_explain(&cli, *pay_date, *gross),
        Command::Stub { pay_date } => cmd_stub(&cli, *pay_date),
        Command::Ytd { year } => cmd_ytd(&cli, *year),
        Command::Owed { year } => cmd_owed(&cli, *year),
        Command::Deposited { pay_date, on } => cmd_mark(&cli, *pay_date, *on, true),
        Command::Remitted { pay_date, on } => cmd_mark(&cli, *pay_date, *on, false),
        Command::Void { pay_date } => cmd_void(&cli, *pay_date),
        Command::Retract { pay_date, record } => cmd_retract(&cli, *pay_date, *record),
        Command::Calendar { year } => cmd_calendar(&cli, *year),
        Command::Check { year } => cmd_check(&cli, *year),
        Command::Form { which } => cmd_form(&cli, which),
        Command::Return { year } => cmd_return(&cli, *year),
    }
}

/// Print one answer as JSON: every command's `--json` form, from the same
/// figures its text form prints.
fn emit_json<T: serde::Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

fn today() -> NaiveDate {
    Local::now().date_naive()
}

fn year_or_current(y: Option<i32>) -> i32 {
    y.unwrap_or_else(|| today().year())
}

struct Loaded {
    config: Config,
    federal: FederalTables,
    state: StateTables,
    ledger: Ledger,
}

fn load(cli: &Cli, year: i32) -> Result<Loaded> {
    let config = Config::load(&cli.config)?;
    let dir = tables::resolve_dir(&cli.config)?;
    let federal = FederalTables::load(&dir, year)?;
    let state = StateTables::load(&dir, &config.company.state, year)?;
    let ledger = Ledger::load(&cli.ledger)?;
    Ok(Loaded {
        config,
        federal,
        state,
        ledger,
    })
}

fn cmd_init(config: &Path, ledger: &Path, json: bool) -> Result<()> {
    for path in [config, ledger] {
        if path.exists() {
            bail!(
                "{} already exists. Delete it first if you mean to start over.",
                path.display()
            );
        }
    }
    std::fs::write(config, Config::template())
        .with_context(|| format!("writing {}", config.display()))?;
    Ledger::default().save(ledger)?;
    if json {
        return emit_json(&serde_json::json!({
            "config": config.display().to_string(),
            "ledger": ledger.display().to_string(),
        }));
    }
    println!("Wrote {} and {}.", config.display(), ledger.display());
    println!("Edit every value in the configuration, then run `rachuba explain` to check the withholding.");
    Ok(())
}
