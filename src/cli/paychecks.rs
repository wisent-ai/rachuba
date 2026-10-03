//! Paycheck computation, withholding explanation and recorded pay stubs.

use super::arguments::Cli;
use super::{emit_json, load, today};
use anyhow::{Context, Result};
use chrono::{Datelike, NaiveDate};
use rachuba::{
    calendar, federal,
    ledger::{self, Ledger},
    money::Cents,
    run, stub,
};

pub(super) fn cmd_run(
    cli: &Cli,
    pay_date: Option<NaiveDate>,
    gross: Option<Cents>,
    kind: &str,
    commit: bool,
) -> Result<()> {
    let pay_date = pay_date.unwrap_or_else(today);
    let l = load(cli, pay_date.year())?;

    let computed = run::compute(&run::RunRequest {
        config: &l.config,
        federal_tables: &l.federal,
        state_tables: &l.state,
        ledger: &l.ledger,
        pay_date,
        gross_override: gross,
        kind: kind.to_string(),
    })?;

    let mut ledger = l.ledger.clone();
    ledger.append(computed.run.clone())?;
    let ytd = ledger.ytd(pay_date.year());
    let schedule = l.config.company.deposit_schedule.into();
    let advisory = l.state.advisory(l.config.company.futa_credit_reduction_ppm);
    if cli.json {
        if commit {
            ledger.save(&cli.ledger)?;
        }
        return emit_json(&serde_json::json!({
            "run": computed.run,
            "notes": computed.notes,
            "advisory": advisory,
            "year_to_date": ytd,
            "form_941_liability": computed.run.form_941_liability(),
            "eftps_due": calendar::employment_tax_deposit_due(schedule, pay_date),
            "elective_deferral": computed.run.elective_deferral(),
            "deferral_due": calendar::deferral_remittance_due(pay_date),
            "recorded": commit,
            "ledger": cli.ledger.display().to_string(),
        }));
    }

    print!("{}", stub::render(&l.config, &computed.run, &ytd));

    for note in &computed.notes {
        println!("\nNOTE  {note}");
    }
    if let Some(a) = advisory {
        println!("\nWARN  {a}");
    }

    println!();
    println!(
        "Deposit {} of Form 941 taxes through EFTPS by {}.",
        computed.run.form_941_liability(),
        calendar::employment_tax_deposit_due(schedule, pay_date)
    );
    if computed.run.elective_deferral().is_positive() {
        println!(
            "Send {} of elective deferral to the plan trustee by {} (seven business days).",
            computed.run.elective_deferral(),
            calendar::deferral_remittance_due(pay_date)
        );
    }

    if commit {
        ledger.save(&cli.ledger)?;
        println!("\nRecorded in {}.", cli.ledger.display());
    } else {
        println!("\nNot recorded. Re-run with --commit to append it to the ledger.");
    }
    Ok(())
}

pub(super) fn cmd_explain(
    cli: &Cli,
    pay_date: Option<NaiveDate>,
    gross: Option<Cents>,
) -> Result<()> {
    let pay_date = pay_date.unwrap_or_else(today);
    let l = load(cli, pay_date.year())?;
    let cfg = &l.config;
    let ytd = l.ledger.ytd_before(pay_date);

    let gross = gross.unwrap_or(cfg.employee.gross_per_period);
    let pretax = cfg.retirement_401k.pretax.applied_to(gross);
    let w = federal::worksheet_1a(&federal::FederalInput {
        tables: &l.federal,
        w4: &cfg.employee.w4,
        frequency: cfg.employee.pay_frequency,
        fit_wages: (gross - pretax).floor_zero(),
        fica_wages: gross,
        futa_wages: gross,
        ytd_ss_wages: ytd.ss_wages,
        ytd_medicare_wages: ytd.fica_wages,
        ytd_futa_wages: ytd.futa_wages,
        futa_credit_reduction_ppm: cfg.company.futa_credit_reduction_ppm,
        round_to_dollar: cfg.company.round_withholding_to_dollar,
    });
    if cli.json {
        return emit_json(&serde_json::json!({
            "worksheet": "1A",
            "publication": format!("Publication 15-T ({})", l.federal.year),
            "source": l.federal.source,
            "lines": w,
        }));
    }
    println!("Worksheet 1A, Publication 15-T ({})", l.federal.year);
    println!("Source: {}\n", l.federal.source);
    println!(
        "  {:<62}{:>12}",
        "1a  Taxable wages this payroll period",
        w.line_1a.to_string()
    );
    println!("  {:<62}{:>12}", "1b  Pay periods per year", w.line_1b);
    println!("  {:<62}{:>12}", "1c  1a x 1b", w.line_1c.to_string());
    println!(
        "  {:<62}{:>12}",
        "1d  Form W-4 Step 4(a), other income",
        w.line_1d.to_string()
    );
    println!("  {:<62}{:>12}", "1e  1c + 1d", w.line_1e.to_string());
    println!(
        "  {:<62}{:>12}",
        "1f  Form W-4 Step 4(b), deductions",
        w.line_1f.to_string()
    );
    println!(
        "  {:<62}{:>12}",
        "1g  Standard deduction for the filing status",
        w.line_1g.to_string()
    );
    println!("  {:<62}{:>12}", "1h  1f + 1g", w.line_1h.to_string());
    println!(
        "  {:<62}{:>12}",
        "1i  Adjusted Annual Wage Amount",
        w.line_1i.to_string()
    );
    println!(
        "  {:<62}{:>12}",
        "2b  Bracket floor (column A)",
        w.line_2b.to_string()
    );
    println!(
        "  {:<62}{:>12}",
        "2c  Bracket base tax (column C)",
        w.line_2c.to_string()
    );
    println!(
        "  {:<62}{:>12}",
        "2d  Bracket rate (column D)",
        format!("{}%", w.line_2d_ppm as f64 / 10_000.0)
    );
    println!("  {:<62}{:>12}", "2e  1i - 2b", w.line_2e.to_string());
    println!("  {:<62}{:>12}", "2f  2e x 2d", w.line_2f.to_string());
    println!(
        "  {:<62}{:>12}",
        "2g  2c + 2f, annual tentative tax",
        w.line_2g.to_string()
    );
    println!(
        "  {:<62}{:>12}",
        "2h  2g / 1b, Tentative Withholding Amount",
        w.line_2h.to_string()
    );
    println!(
        "  {:<62}{:>12}",
        "3a  Form W-4 Step 3, credits",
        w.line_3a.to_string()
    );
    println!("  {:<62}{:>12}", "3b  3a / 1b", w.line_3b.to_string());
    println!("  {:<62}{:>12}", "3c  2h - 3b", w.line_3c.to_string());
    println!(
        "  {:<62}{:>12}",
        "4a  Form W-4 Step 4(c), extra withholding",
        w.line_4a.to_string()
    );
    println!(
        "  {:<62}{:>12}",
        "4b  Federal income tax to withhold",
        w.line_4b.to_string()
    );
    Ok(())
}

pub(super) fn cmd_stub(cli: &Cli, pay_date: NaiveDate) -> Result<()> {
    let l = load(cli, pay_date.year())?;
    let run = l
        .ledger
        .runs
        .iter()
        .find(|r| r.pay_date == pay_date)
        .with_context(|| format!("no recorded run with pay date {pay_date}"))?;
    let mut ytd = ledger::YearToDate::default();
    let mut acc = Ledger::default();
    for r in l.ledger.runs_in_year(pay_date.year()) {
        acc.runs.push(r.clone());
        if r.pay_date == pay_date {
            ytd = acc.ytd(pay_date.year());
            break;
        }
    }
    if cli.json {
        return emit_json(&serde_json::json!({ "run": run, "year_to_date": ytd }));
    }
    print!("{}", stub::render(&l.config, run, &ytd));
    Ok(())
}

/// `void`: take the last recorded run back out of the ledger.
pub(super) fn cmd_void(cli: &Cli, pay_date: NaiveDate) -> Result<()> {
    let mut ledger = Ledger::load(&cli.ledger)?;
    let run = ledger.void(pay_date)?;
    ledger.save(&cli.ledger)?;
    if cli.json {
        return emit_json(&serde_json::json!({
            "voided": run, "ledger": cli.ledger.display().to_string(),
        }));
    }
    println!(
        "Voided the {} run on {pay_date} ({} gross); {} no longer records it.",
        run.kind,
        run.gross,
        cli.ledger.display()
    );
    Ok(())
}
