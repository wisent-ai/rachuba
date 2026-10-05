//! `rachuba return`: the employee's own federal return for a year, projected
//! from the ledger, the remaining pay periods and the household section.

use anyhow::{Context, Result};
use rachuba::{
    money::Cents,
    personal::{self, IraEligibility, Projection, ReturnTables},
    tables,
};

use crate::cli::arguments::Cli;
use crate::cli::{emit_json, load, today, year_or_current};

/// Parts per million in one percentage point.
const PPM_PER_PERCENT: i64 = 10_000;

pub(in crate::cli) fn cmd_return(cli: &Cli, year: Option<i32>) -> Result<()> {
    let year = year_or_current(year);
    let l = load(cli, year)?;
    let household = l.config.household.as_ref().with_context(|| {
        format!(
            "{} has no [household] section, so there is no return to project: the filing status \
             and the household's income outside this payroll are unknown. Copy the [household] \
             section of the file `rachuba init` writes into it and set every value.",
            cli.config.display()
        )
    })?;
    let dir = tables::resolve_dir(&cli.config)?;
    let return_tables = ReturnTables::load(&dir, year)?;
    let p = personal::project(&personal::ProjectionRequest {
        year,
        config: &l.config,
        household,
        federal: &l.federal,
        state: &l.state,
        ledger: &l.ledger,
        tables: &return_tables,
        today: today(),
    })?;
    if cli.json {
        return emit_json(&p);
    }
    print_projection(&p);
    Ok(())
}

fn row(label: &str, value: Cents) {
    println!("  {:<52}{:>14}", label, value.to_string());
}

fn percent(ppm: i64) -> String {
    let (whole, rest) = (ppm / PPM_PER_PERCENT, ppm % PPM_PER_PERCENT);
    if rest == 0 {
        format!("{whole}%")
    } else {
        format!("{whole}.{}%", format!("{rest:04}").trim_end_matches('0'))
    }
}

fn print_projection(p: &Projection) {
    let t = &p.tax;
    println!(
        "Federal return projection for {}, {}\n",
        p.year, p.filing_status
    );
    println!(
        "  {} regular run(s) recorded; {} remaining pay period(s) projected at the configured\n  \
         gross and elections.\n",
        p.payroll.recorded_runs, p.payroll.remaining_periods
    );

    let i = &p.income;
    println!("Income");
    row("Wages from this payroll (W-2 box 1)", p.payroll.w2_box1);
    if p.ira_spouse.is_some() {
        row("Spouse wages", i.wages - p.payroll.w2_box1);
    }
    row("Other income", i.other_income);
    row(
        "Interest and ordinary dividends",
        i.interest_and_ordinary_dividends,
    );
    row("Qualified dividends", i.qualified_dividends);
    row(
        &format!(
            "Capital gain or loss (short {}, long {})",
            i.short_term_capital_gains, i.long_term_capital_gains
        ),
        t.capital_gain_in_income,
    );
    row("Adjustments to income", -i.adjustments);
    row("Adjusted gross income", t.adjusted_gross_income);
    row(&format!("Deduction ({})", t.deduction_kind), -t.deduction);
    row("Taxable income", t.taxable_income);
    println!();

    println!("Tax");
    row("Income tax", t.income_tax);
    if t.preferential_income.is_positive() {
        row(
            "  of which income taxed at capital gains rates",
            t.preferential_income,
        );
    }
    row("Credits", -t.credits_applied);
    row("Net investment income tax", t.net_investment_income_tax);
    row("Additional Medicare Tax", t.additional_medicare_tax);
    row("Total tax", t.total_tax);
    println!(
        "  {:<52}{:>14}",
        "Bracket of the last dollar of ordinary income",
        percent(t.ordinary_bracket_ppm)
    );
    println!();

    let e = &p.estimated;
    println!("Payments");
    row("Withheld by this payroll", p.payroll.federal_withholding);
    row("Withheld elsewhere", p.other_withholding);
    row("Estimated tax payments", e.estimated_payments);
    if e.balance_due.is_positive() {
        row("Balance due with the return", e.balance_due);
    } else {
        row("Refund", -e.balance_due);
    }
    println!();

    println!("Estimated tax");
    row("Required annual payment", e.required_annual_payment);
    row("  90% of this year's tax", e.current_year_share);
    match (e.prior_year_share, e.prior_year_ppm) {
        (Some(share), Some(ppm)) => row(&format!("  {} of last year's tax", percent(ppm)), share),
        _ => println!("  Last year's tax is not in [household], so only the 90% rule applies."),
    }
    println!(
        "\n  {:<5}{:<13}{:>16}{:>16}{:>16}",
        "No.", "Due", "Required", "Credited", "Short"
    );
    for i in &e.installments {
        println!(
            "  {:<5}{:<13}{:>16}{:>16}{:>16}",
            i.number,
            i.due.to_string(),
            i.required_to_date.to_string(),
            i.credited_to_date.to_string(),
            i.shortfall.to_string()
        );
    }
    if e.below_minimum {
        println!(
            "\n  Tax after withholding is under the minimum: no estimated tax addition applies."
        );
    } else if let Some(next) = &e.next_due {
        if next.shortfall.is_positive() {
            println!(
                "\n  Installment {} is due {}: paying {} by then clears the shortfall at that date.",
                next.number, next.due, next.shortfall
            );
        } else {
            println!(
                "\n  Installment {} is due {} and is already covered.",
                next.number, next.due
            );
        }
    }
    println!();

    let r = &p.retirement;
    println!("401(k)");
    row("Elective deferral ceiling", r.deferral_ceiling);
    row("Deferred so far", r.deferred_to_date);
    row("Deferred for the year as elected", r.projected_deferral);
    row("Ceiling unused after that", r.room_after_projection);
    if let Some(per) = r.per_period_to_fill {
        row("Deferral per remaining period that reaches it", per);
    }
    row(
        "Largest employer contribution for the year",
        r.employer_maximum,
    );
    row("Employer contribution as elected", r.employer_projected);
    row(
        "Federal tax saved by the pre-tax deferrals",
        r.pretax_saving,
    );
    row(
        "Further saving if the unused ceiling goes pre-tax",
        r.fill_room_saving,
    );
    println!();

    row("Modified AGI used for IRA phase-outs", p.ira_magi);
    print_ira("IRA, employee", &p.ira_employee);
    if let Some(spouse) = &p.ira_spouse {
        print_ira("IRA, spouse", spouse);
    }
    println!("State income tax is not part of this projection.");
    println!();
    println!("Tables for {}", p.year);
    println!("  Payroll         {}", p.tables.payroll);
    println!("  Individual      {}", p.tables.individual);
    println!("  IRA             {}", p.tables.ira);
    println!("  Legal holidays  {}", p.tables.legal_holidays);
}

fn print_ira(title: &str, i: &IraEligibility) {
    println!("{title}");
    row("Contribution limit", i.limit);
    let deduction = match i.deduction_range {
        Some((low, high)) => format!("Traditional IRA deduction (phase-out {low} to {high})"),
        None => "Traditional IRA deduction (no phase-out applies)".to_string(),
    };
    row(&deduction, i.traditional_deduction);
    let (low, high) = i.roth_range;
    row(
        &format!("Roth IRA contribution (phase-out {low} to {high})"),
        i.roth_contribution,
    );
    if i.roth_contribution < i.limit {
        println!(
            "  A nondeductible traditional IRA contribution is still allowed up to the limit, and\n  \
             can be converted to a Roth IRA; other pre-tax IRA balances make part of the\n  \
             conversion taxable."
        );
    }
    println!();
}
