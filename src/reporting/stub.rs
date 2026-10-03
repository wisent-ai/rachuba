//! Pay stub rendering.
//!
//! New York Labor Law section 195(3) requires a wage statement with each
//! payment showing the dates covered, the rate and basis of pay, gross wages,
//! each deduction itemised, and net wages. Most states impose a comparable
//! requirement. The layout below shows each of those, plus a year-to-date
//! column, plus the employer-side costs that never appear on the employee's
//! side of the ledger but which the company needs to see.

use std::fmt::Write;

use crate::config::Config;
use crate::ledger::{PayrollRun, YearToDate};
use crate::money::Cents;

const WIDTH: usize = 68;

/// Render the stub for one run, with year-to-date figures that include it.
pub fn render(config: &Config, run: &PayrollRun, ytd: &YearToDate) -> String {
    let mut o = String::with_capacity(2048);
    let e = &config.employee;

    rule(&mut o, '=');
    let _ = writeln!(o, "{}", center(&config.company.legal_name));
    let _ = writeln!(o, "{}", center(&config.company.address));
    let _ = writeln!(o, "{}", center(&format!("EIN {}", config.company.ein)));
    rule(&mut o, '=');
    let _ = writeln!(
        o,
        "{:<34}{:>34}",
        e.name,
        format!("Pay date  {}", run.pay_date)
    );
    let _ = writeln!(
        o,
        "{:<34}{:>32}",
        format!("SSN xxx-xx-{}", e.ssn_last4),
        format!("Frequency  {}", e.pay_frequency.label())
    );
    let _ = writeln!(o, "{:<34}{:>34}", e.address, format!("Type  {}", run.kind));
    let _ = writeln!(o);
    let _ = writeln!(o, "  {:<40}{:>12}{:>14}", "", "CURRENT", "YEAR TO DATE");

    section(&mut o, "EARNINGS");
    line(&mut o, "Gross wages", run.gross, ytd.gross);

    if run.pretax_401k.is_positive() || run.roth_401k.is_positive() {
        section(&mut o, "PRE-TAX AND RETIREMENT DEDUCTIONS");
        if run.pretax_401k.is_positive() {
            line(
                &mut o,
                "401(k) elective deferral, pre-tax",
                run.pretax_401k,
                ytd.pretax_401k,
            );
        }
        if run.roth_401k.is_positive() {
            line(
                &mut o,
                "401(k) designated Roth",
                run.roth_401k,
                ytd.roth_401k,
            );
        }
    }

    section(&mut o, "TAXABLE WAGES");
    line(
        &mut o,
        "Federal income tax wages",
        run.fit_wages,
        ytd.fit_wages,
    );
    line(
        &mut o,
        "Social Security wages",
        run.ss_taxable_wages,
        ytd.ss_wages,
    );
    line(&mut o, "Medicare wages", run.fica_wages, ytd.fica_wages);

    section(&mut o, "TAXES WITHHELD");
    line(
        &mut o,
        "Federal income tax",
        run.federal_income_tax,
        ytd.federal_income_tax,
    );
    line(&mut o, "Social Security", run.ss_employee, ytd.ss_employee);
    line(
        &mut o,
        "Medicare",
        run.medicare_employee,
        ytd.medicare_employee,
    );
    if run.additional_medicare.is_positive() || ytd.additional_medicare.is_positive() {
        line(
            &mut o,
            "Additional Medicare",
            run.additional_medicare,
            ytd.additional_medicare,
        );
    }
    if run.state_income_tax.is_positive() || ytd.state_income_tax.is_positive() {
        line(
            &mut o,
            &format!("{} state income tax", config.company.state),
            run.state_income_tax,
            ytd.state_income_tax,
        );
    }
    if run.local_income_tax.is_positive() || ytd.local_income_tax.is_positive() {
        line(
            &mut o,
            "Local income tax",
            run.local_income_tax,
            ytd.local_income_tax,
        );
    }
    if run.sdi_employee.is_positive() || ytd.sdi_employee.is_positive() {
        line(
            &mut o,
            "State disability insurance",
            run.sdi_employee,
            ytd.sdi_employee,
        );
    }
    if run.pfl_employee.is_positive() || ytd.pfl_employee.is_positive() {
        line(
            &mut o,
            "Paid family leave",
            run.pfl_employee,
            ytd.pfl_employee,
        );
    }

    let withheld = run.federal_income_tax
        + run.ss_employee
        + run.medicare_employee
        + run.additional_medicare
        + run.state_income_tax
        + run.local_income_tax
        + run.state_employee_insurance();
    let ytd_withheld = ytd.federal_income_tax
        + ytd.ss_employee
        + ytd.medicare_employee
        + ytd.additional_medicare
        + ytd.state_income_tax
        + ytd.local_income_tax
        + ytd.state_employee_insurance();
    rule(&mut o, '-');
    line(&mut o, "Total withheld", withheld, ytd_withheld);

    rule(&mut o, '=');
    line(&mut o, "NET PAY", run.net_pay, ytd.net_pay);
    rule(&mut o, '=');

    // Employer side. Never part of the employee's pay, always part of what the
    // run costs the company.
    section(&mut o, "EMPLOYER CONTRIBUTIONS (not withheld from pay)");
    line(&mut o, "Social Security", run.ss_employer, ytd.ss_employer);
    line(
        &mut o,
        "Medicare",
        run.medicare_employer,
        ytd.medicare_employer,
    );
    if run.futa_employer.is_positive() || ytd.futa_employer.is_positive() {
        line(
            &mut o,
            "Federal unemployment (FUTA)",
            run.futa_employer,
            ytd.futa_employer,
        );
    }
    if run.sui_employer.is_positive() || ytd.sui_employer.is_positive() {
        line(
            &mut o,
            "State unemployment (SUI)",
            run.sui_employer,
            ytd.sui_employer,
        );
    }
    if run.state_training_tax.is_positive() || ytd.state_training_tax.is_positive() {
        line(
            &mut o,
            "State employment training tax",
            run.state_training_tax,
            ytd.state_training_tax,
        );
    }
    if run.employer_401k.is_positive() || ytd.employer_401k.is_positive() {
        line(
            &mut o,
            "401(k) employer contribution",
            run.employer_401k,
            ytd.employer_401k,
        );
    }
    rule(&mut o, '-');
    line(
        &mut o,
        "Total cost of this run",
        run.employer_cost(),
        ytd.employer_cost(),
    );

    o
}

fn rule(o: &mut String, ch: char) {
    let _ = writeln!(o, "{}", std::iter::repeat_n(ch, WIDTH).collect::<String>());
}

fn section(o: &mut String, title: &str) {
    let _ = writeln!(o);
    let _ = writeln!(o, "{title}");
}

fn line(o: &mut String, label: &str, current: Cents, ytd: Cents) {
    let ytd_cell = if ytd.is_zero() {
        String::new()
    } else {
        ytd.to_string()
    };
    let _ = writeln!(
        o,
        "  {:<40}{:>12}{:>14}",
        label,
        current.to_string(),
        ytd_cell
    );
}

fn center(s: &str) -> String {
    if s.len() >= WIDTH {
        return s.to_string();
    }
    let pad = (WIDTH - s.len()) / 2;
    format!("{}{}", " ".repeat(pad), s)
}
