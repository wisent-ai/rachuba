//! Year-to-date summaries, federal filing line values, and the projection of
//! the employee's own return.

mod federal_return;
pub(super) use federal_return::cmd_return;

use super::arguments::{Cli, FormKind};
use super::{emit_json, load, year_or_current};
use anyhow::{bail, Result};
use rachuba::{
    calendar,
    forms::{Form940, Form941, FormW2},
    money::Cents,
};

pub(super) fn cmd_ytd(cli: &Cli, year: Option<i32>) -> Result<()> {
    let year = year_or_current(year);
    let l = load(cli, year)?;
    let y = l.ledger.ytd(year);
    if cli.json {
        return emit_json(&serde_json::json!({ "year": year, "runs": y.runs, "totals": y }));
    }
    println!("Year to date {year}, {} run(s)\n", y.runs);
    let row = |label: &str, v: Cents| println!("  {:<44}{:>14}", label, v.to_string());
    row("Gross wages", y.gross);
    row("401(k) pre-tax deferral", y.pretax_401k);
    row("401(k) Roth deferral", y.roth_401k);
    row("401(k) employer contribution", y.employer_401k);
    println!();
    row("Federal income tax withheld", y.federal_income_tax);
    row("Social Security withheld", y.ss_employee);
    row("Medicare withheld", y.medicare_employee);
    row("Additional Medicare withheld", y.additional_medicare);
    row("State income tax withheld", y.state_income_tax);
    row("Local income tax withheld", y.local_income_tax);
    row("State disability withheld", y.sdi_employee);
    row("Paid family leave withheld", y.pfl_employee);
    println!();
    row("Net pay", y.net_pay);
    println!();
    row("Employer Social Security", y.ss_employer);
    row("Employer Medicare", y.medicare_employer);
    row("Employer FUTA", y.futa_employer);
    row("Employer SUI", y.sui_employer);
    row("Employer training tax", y.state_training_tax);
    Ok(())
}

pub(super) fn cmd_form(cli: &Cli, which: &FormKind) -> Result<()> {
    match which {
        FormKind::F941 { year, quarter } => {
            let year = year_or_current(*year);
            if !(1..=calendar::QUARTERS_PER_YEAR).contains(quarter) {
                bail!("quarter must be 1, 2, 3 or {}", calendar::QUARTERS_PER_YEAR);
            }
            let l = load(cli, year)?;
            let f = Form941::build(&l.ledger, &l.federal, year, *quarter);
            let due = calendar::form_941_due(&l.federal.legal_holidays, year, *quarter)?;
            if cli.json {
                return emit_json(&serde_json::json!({
                    "form": "941", "due": due, "values": f,
                }));
            }
            println!("Form 941, {} Q{}\n", f.year, f.quarter);
            println!(
                "  {:<6}{:<52}{:>12}",
                "1", "Number of employees", f.employees
            );
            for line in &f.lines {
                println!(
                    "  {:<6}{:<52}{:>12}",
                    line.number,
                    line.label,
                    line.value.plain()
                );
            }
            println!();
            if f.balance_due.is_positive() {
                println!(
                    "  {:<6}{:<52}{:>12}",
                    "14",
                    "Balance due",
                    f.balance_due.plain()
                );
            } else {
                println!(
                    "  {:<6}{:<52}{:>12}",
                    "15",
                    "Overpayment",
                    f.overpayment.plain()
                );
            }
            println!("\n  Due {due}.");
        }
        FormKind::F940 { year } => {
            let year = year_or_current(*year);
            let l = load(cli, year)?;
            let f = Form940::build(&l.ledger, &l.federal, year);
            let due = calendar::form_940_due(&l.federal.legal_holidays, year)?;
            if cli.json {
                return emit_json(&serde_json::json!({
                    "form": "940", "due": due, "values": f,
                }));
            }
            println!("Form 940, {}\n", f.year);
            for line in &f.lines {
                println!(
                    "  {:<6}{:<52}{:>12}",
                    line.number,
                    line.label,
                    line.value.plain()
                );
            }
            if f.part5_required {
                println!("\n  Part 5, quarterly liabilities:");
                for (i, q) in f.quarterly.iter().enumerate() {
                    println!(
                        "  {:<6}{:<52}{:>12}",
                        format!("16{}", (b'a' + i as u8) as char),
                        format!("Quarter {}", i + 1),
                        q.plain()
                    );
                }
            } else {
                println!("\n  Part 5 not required: the annual tax is at or below the threshold.");
            }
            println!("\n  Due {due}.");
        }
        FormKind::W2 { year } => {
            let year = year_or_current(*year);
            let l = load(cli, year)?;
            let locality = if l.config.employee.has_flag("nyc_resident") {
                Some("New York City".to_string())
            } else if l.config.employee.has_flag("yonkers_resident") {
                Some("Yonkers".to_string())
            } else {
                None
            };
            let w2 = FormW2::build(
                &l.ledger,
                &l.federal,
                year,
                &l.config.company.state,
                locality,
            );
            let due = calendar::form_w2_due(&l.federal.legal_holidays, year)?;
            if cli.json {
                return emit_json(&serde_json::json!({
                    "form": "W-2",
                    "due": due,
                    "employee": l.config.employee.name,
                    "ssn_last4": l.config.employee.ssn_last4,
                    "employer": l.config.company.legal_name,
                    "ein": l.config.company.ein,
                    "values": w2,
                }));
            }
            println!("Form W-2, {}\n", w2.year);
            println!(
                "  Employee: {} (SSN xxx-xx-{})",
                l.config.employee.name, l.config.employee.ssn_last4
            );
            println!(
                "  Employer: {}, EIN {}\n",
                l.config.company.legal_name, l.config.company.ein
            );
            for b in &w2.boxes {
                println!("  {:<6}{:<52}{:>12}", b.number, b.label, b.value.plain());
            }
            for (code, label, value) in &w2.box12 {
                println!(
                    "  {:<6}{:<52}{:>12}",
                    format!("12 {code}"),
                    label,
                    value.plain()
                );
            }
            println!(
                "  {:<6}{:<52}{:>12}",
                "13",
                "Retirement plan",
                if w2.retirement_plan_checked {
                    "checked"
                } else {
                    "-"
                }
            );
            println!("  {:<6}{:<52}{:>12}", "15", "State", w2.state_code);
            if let Some(loc) = &w2.locality {
                println!("  {:<6}{:<52}{:>12}", "20", "Locality name", loc);
            }
            println!(
                "\n  Due {due}. File at SSA Business Services Online; the full Social Security \
                 number is entered there, not stored here."
            );
        }
    }
    Ok(())
}
