//! Deposit and remittance records, due dates and statutory checks.

use super::arguments::{Cli, Record};
use super::{emit_json, load, today, year_or_current};
use anyhow::{bail, Context, Result};
use chrono::NaiveDate;
use rachuba::{
    calendar,
    config::Config,
    forms::{self, Severity},
    ledger::Ledger,
};

pub(super) fn cmd_owed(cli: &Cli, year: Option<i32>) -> Result<()> {
    let year = year_or_current(year);
    let l = load(cli, year)?;
    let schedule = l.config.company.deposit_schedule.into();
    let findings = forms::outstanding_obligations(&l.ledger, year, schedule, today());
    let overdue = findings.iter().any(|f| f.severity == Severity::Error);
    if cli.json {
        emit_json(&serde_json::json!({ "year": year, "findings": findings, "overdue": overdue }))?;
        if overdue {
            bail!("at least one due date has passed");
        }
        return Ok(());
    }
    if findings.is_empty() {
        println!("Nothing outstanding. Every run is recorded as deposited and remitted.");
        return Ok(());
    }
    for f in &findings {
        println!("{:<6}{}", f.severity.tag(), f.message);
    }
    if overdue {
        bail!("at least one due date has passed");
    }
    Ok(())
}

pub(super) fn cmd_mark(
    cli: &Cli,
    pay_date: NaiveDate,
    on: Option<NaiveDate>,
    deposit: bool,
) -> Result<()> {
    let on = on.unwrap_or_else(today);
    let mut ledger = Ledger::load(&cli.ledger)?;
    let run = ledger
        .runs
        .iter_mut()
        .find(|r| r.pay_date == pay_date)
        .with_context(|| format!("no recorded run with pay date {pay_date}"))?;

    let (recorded, amount, due, late_notice) = if deposit {
        run.taxes_deposited = Some(on);
        let due = calendar::employment_tax_deposit_due(
            Config::load(&cli.config)?.company.deposit_schedule.into(),
            pay_date,
        );
        (
            "deposit",
            run.form_941_liability(),
            due,
            format!("This deposit was due {due}. Expect a failure-to-deposit penalty."),
        )
    } else {
        run.deferral_remitted = Some(on);
        let due = calendar::deferral_remittance_due(pay_date);
        (
            "remittance",
            run.elective_deferral(),
            due,
            format!(
                "The safe harbor closed {due}. This is a prohibited transaction; correct it \
                 through the DOL Voluntary Fiduciary Correction Program."
            ),
        )
    };
    let late = on > due;
    if cli.json {
        ledger.save(&cli.ledger)?;
        return emit_json(&serde_json::json!({
            "recorded": recorded, "pay_date": pay_date, "amount": amount, "on": on,
            "due": due, "late": late, "ledger": cli.ledger.display().to_string(),
        }));
    }
    if deposit {
        println!("Recorded EFTPS deposit of {amount} on {on}.");
    } else {
        println!("Recorded deferral remittance of {amount} on {on}.");
    }
    if late {
        println!("LATE  {late_notice}");
    }
    ledger.save(&cli.ledger)?;
    Ok(())
}

/// `retract`: take back a `deposited` or `remitted` record, so `owed` reports
/// that money as still due.
pub(super) fn cmd_retract(cli: &Cli, pay_date: NaiveDate, record: Record) -> Result<()> {
    let mut ledger = Ledger::load(&cli.ledger)?;
    let run = ledger
        .runs
        .iter_mut()
        .find(|r| r.pay_date == pay_date)
        .with_context(|| format!("no recorded run with pay date {pay_date}"))?;
    let (name, slot) = match record {
        Record::Deposit => ("deposit", &mut run.taxes_deposited),
        Record::Remittance => ("remittance", &mut run.deferral_remitted),
    };
    let Some(on) = slot.take() else {
        bail!("the run on {pay_date} records no {name} to retract");
    };
    ledger.save(&cli.ledger)?;
    if cli.json {
        return emit_json(&serde_json::json!({
            "retracted": name, "pay_date": pay_date, "was_on": on,
            "ledger": cli.ledger.display().to_string(),
        }));
    }
    println!("Retracted the {name} recorded on {on} for the run on {pay_date}; `owed` reports it as due again.");
    Ok(())
}

pub(super) fn cmd_calendar(cli: &Cli, year: Option<i32>) -> Result<()> {
    let year = year_or_current(year);
    let l = load(cli, year)?;
    let schedule = l.config.company.deposit_schedule.into();

    if cli.json {
        let quarters: Vec<serde_json::Value> = (1..=calendar::QUARTERS_PER_YEAR)
            .map(|q| {
                serde_json::json!({
                    "quarter": q,
                    "ends": calendar::quarter_end(year, q),
                    "form_941_due": calendar::form_941_due(year, q),
                })
            })
            .collect();
        let runs: Vec<serde_json::Value> = l
            .ledger
            .runs_in_year(year)
            .map(|r| {
                serde_json::json!({
                    "pay_date": r.pay_date,
                    "eftps_due": calendar::employment_tax_deposit_due(schedule, r.pay_date),
                    "deferral_due": calendar::deferral_remittance_due(r.pay_date),
                })
            })
            .collect();
        return emit_json(&serde_json::json!({
            "year": year,
            "quarters": quarters,
            "form_940_due": calendar::form_940_due(year),
            "form_w2_due": calendar::form_w2_due(year),
            "runs": runs,
        }));
    }
    println!("Due dates for {year}\n");
    for q in 1..=calendar::QUARTERS_PER_YEAR {
        println!(
            "  Q{q} ends {}   Form 941 due {}",
            calendar::quarter_end(year, q),
            calendar::form_941_due(year, q)
        );
    }
    println!();
    println!("  Form 940 for {year} due {}", calendar::form_940_due(year));
    println!(
        "  Forms W-2 and W-3 for {year} due {}",
        calendar::form_w2_due(year)
    );
    println!();
    println!("Per-run due dates for the runs recorded so far:\n");
    for r in l.ledger.runs_in_year(year) {
        println!(
            "  {}  EFTPS by {}   deferral by {}",
            r.pay_date,
            calendar::employment_tax_deposit_due(schedule, r.pay_date),
            calendar::deferral_remittance_due(r.pay_date)
        );
    }
    Ok(())
}

pub(super) fn cmd_check(cli: &Cli, year: Option<i32>) -> Result<()> {
    let year = year_or_current(year);
    let l = load(cli, year)?;
    let findings = forms::check_limits(
        &l.ledger,
        &l.federal,
        year,
        l.config.employee.age_this_year,
        l.config.retirement_401k.prior_year_ss_wages,
    );
    let advisory = l.state.advisory(l.config.company.futa_credit_reduction_ppm);
    let breached = findings.iter().any(|f| f.severity == Severity::Error);
    if cli.json {
        emit_json(&serde_json::json!({
            "year": year, "findings": findings, "advisory": advisory, "breached": breached,
        }))?;
    } else {
        for f in &findings {
            println!("{:<6}{}", f.severity.tag(), f.message);
        }
        if let Some(a) = advisory {
            println!("{:<6}{}", "WARN", a);
        }
    }
    if breached {
        bail!("a statutory limit has been breached");
    }
    Ok(())
}
