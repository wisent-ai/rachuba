//! `rachuba runs`: every recorded run of a year, oldest first, with whether
//! the money it owes has moved: the record a person reads before `void`,
//! `deposited` or `remitted` to find the pay date to name.

use super::arguments::Cli;
use super::{emit_json, load, year_or_current};
use anyhow::Result;
use rachuba::money::Cents;
use serde::Serialize;

/// One recorded run as `runs` lists it.
#[derive(Serialize)]
struct Listed {
    pay_date: chrono::NaiveDate,
    kind: String,
    gross: Cents,
    net_pay: Cents,
    /// The day its Form 941 taxes were deposited; `null` while they were not.
    taxes_deposited: Option<chrono::NaiveDate>,
    /// The day its elective deferral reached the plan; `null` while it did
    /// not, and for a run with no deferral.
    deferral_remitted: Option<chrono::NaiveDate>,
    /// Whether the run deferred anything, so a missing remittance is owed.
    deferred: bool,
}

/// A recorded date, or what is still owed in its place.
fn moved(on: Option<chrono::NaiveDate>, owed: bool, what: &str) -> String {
    match (on, owed) {
        (Some(day), _) => format!("{what} {day}"),
        (None, true) => format!("{what} pending"),
        (None, false) => format!("nothing to {what}"),
    }
}

pub(super) fn cmd_runs(cli: &Cli, year: Option<i32>, kind: Option<&str>, pending: bool) -> Result<()> {
    let year = year_or_current(year);
    let wanted = kind.map(|text| text.trim().to_lowercase());
    let l = load(cli, year)?;
    let listed: Vec<Listed> = l
        .ledger
        .runs_in_year(year)
        .filter(|run| wanted.as_ref().is_none_or(|kind| run.kind.to_lowercase() == *kind))
        .map(|run| Listed {
            pay_date: run.pay_date,
            kind: run.kind.clone(),
            gross: run.gross,
            net_pay: run.net_pay,
            taxes_deposited: run.taxes_deposited,
            deferral_remitted: run.deferral_remitted,
            deferred: run.pretax_401k.0.is_positive() || run.roth_401k.0.is_positive(),
        })
        .filter(|run| !pending || run.taxes_deposited.is_none() || (run.deferred && run.deferral_remitted.is_none()))
        .collect();
    if cli.json {
        return emit_json(&serde_json::json!({ "year": year, "kind": kind, "pending": pending, "runs": listed }));
    }
    let mut named = match kind {
        Some(wanted) => format!(" of kind {wanted}"),
        None => String::new(),
    };
    if pending {
        named.push_str(" with money still to move");
    }
    if listed.is_empty() {
        println!("No runs{named} recorded in {year}");
        return Ok(());
    }
    println!("Runs{named} recorded in {year}\n");
    for run in &listed {
        println!(
            "  {}  {}  gross {}  net {}  {}  {}",
            run.pay_date,
            run.kind,
            run.gross,
            run.net_pay,
            moved(run.taxes_deposited, true, "deposited"),
            moved(run.deferral_remitted, run.deferred, "remitted"),
        );
    }
    Ok(())
}

/// One CSV cell, quoted as RFC 4180 quotes every field: an amount stays the
/// decimal string the ledger keeps, and an unrecorded date is left empty.
fn cell(value: Option<&serde_json::Value>) -> String {
    let text = match value {
        Some(serde_json::Value::String(text)) => text.clone(),
        Some(serde_json::Value::Null) | None => String::new(),
        Some(other) => other.to_string(),
    };
    format!("\"{}\"", text.replace('"', "\"\""))
}

/// `rachuba sheet`: every recorded run of a year as one CSV sheet for a
/// spreadsheet or an accountant, a row per run, oldest first. The columns are
/// the fields the ledger records for a run, by the names it stores them
/// under, so a field the ledger gains is a column without a change here.
pub(super) fn cmd_sheet(cli: &Cli, year: Option<i32>) -> Result<()> {
    let year = year_or_current(year);
    let ledger = rachuba::ledger::Ledger::load(&cli.ledger)?;
    let rows: Vec<serde_json::Map<String, serde_json::Value>> = ledger
        .runs_in_year(year)
        .map(|run| match serde_json::to_value(run)? {
            serde_json::Value::Object(fields) => Ok(fields),
            other => anyhow::bail!("the run paid {} is recorded as {other}, not as fields", run.pay_date),
        })
        .collect::<Result<_>>()?;
    let columns: std::collections::BTreeSet<&String> = rows.iter().flat_map(|row| row.keys()).collect();
    // A year with no run has no fields to name: the sheet is empty, not a blank header.
    if columns.is_empty() {
        return Ok(());
    }
    let header: Vec<String> = columns.iter().map(|name| format!("\"{name}\"")).collect();
    println!("{}", header.join(","));
    for row in &rows {
        let cells: Vec<String> = columns.iter().map(|name| cell(row.get(*name))).collect();
        println!("{}", cells.join(","));
    }
    Ok(())
}

/// One month's payroll cost to the company.
#[derive(Serialize, Default)]
struct MonthCost {
    month: String,
    runs: usize,
    gross: Cents,
    /// Social Security, Medicare, FUTA, state unemployment and training tax
    /// the company pays on top of the wages.
    employer_taxes: Cents,
    employer_401k: Cents,
    /// Everything the company pays out for the month's runs: `employer_cost`
    /// summed, the figure the pay stub prints as the cost of a run.
    total: Cents,
}

/// `rachuba cost`: what payroll cost the company in each month of a year
/// with a run, by the month the runs were paid in, and in the whole year:
/// the figure to plan cash against, which `ytd` gives only for the year.
pub(super) fn cmd_cost(cli: &Cli, year: Option<i32>) -> Result<()> {
    let year = year_or_current(year);
    let ledger = rachuba::ledger::Ledger::load(&cli.ledger)?;
    let mut months: std::collections::BTreeMap<String, MonthCost> = std::collections::BTreeMap::new();
    for run in ledger.runs_in_year(year) {
        let month = run.pay_date.format("%Y-%m").to_string();
        let cost = months.entry(month.clone()).or_insert_with(|| MonthCost { month, ..MonthCost::default() });
        cost.runs += std::slice::from_ref(run).len();
        cost.gross += run.gross;
        cost.employer_taxes += run.ss_employer + run.medicare_employer + run.futa_employer + run.sui_employer + run.state_training_tax;
        cost.employer_401k += run.employer_401k;
        cost.total += run.employer_cost();
    }
    let months: Vec<MonthCost> = months.into_values().collect();
    let year_total = MonthCost {
        month: year.to_string(),
        runs: months.iter().map(|month| month.runs).sum(),
        gross: months.iter().map(|month| month.gross).sum(),
        employer_taxes: months.iter().map(|month| month.employer_taxes).sum(),
        employer_401k: months.iter().map(|month| month.employer_401k).sum(),
        total: months.iter().map(|month| month.total).sum(),
    };
    if cli.json {
        return emit_json(&serde_json::json!({ "year": year, "months": months, "total": year_total }));
    }
    if months.is_empty() {
        println!("No runs recorded in {year}");
        return Ok(());
    }
    println!("What payroll cost in {year}\n");
    for cost in months.iter().chain(std::iter::once(&year_total)) {
        println!(
            "  {}  {} run(s)  gross {}  employer taxes {}  employer 401(k) {}  total {}",
            cost.month, cost.runs, cost.gross, cost.employer_taxes, cost.employer_401k, cost.total,
        );
    }
    Ok(())
}
