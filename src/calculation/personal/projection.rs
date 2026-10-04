//! The projected return: the ledger's year, completed with the pay periods
//! still to come, set against the household's other income.
//!
//! The year is completed by computing every remaining regular pay period
//! with the engine that computes a real one, at the configured gross and
//! elections, so the projected withholding, deferrals and employer
//! contribution obey the same limits a real run would. The remaining count is
//! the periods in a year less the regular runs already recorded; the
//! projected runs are dated at the end of the year and never recorded.

use anyhow::{bail, Result};
use chrono::{Datelike, Days, NaiveDate};
use serde::Serialize;

use super::estimated::{estimated_tax, EstimatedInput};
use super::{
    compute_tax, EstimatedTax, IraEligibility, ReturnIncome, ReturnTables, TaxComputation,
};
use crate::calendar::{estimated_tax_due, ESTIMATED_TAX_INSTALLMENTS};
use crate::config::{Config, Household};
use crate::ledger::Ledger;
use crate::money::Cents;
use crate::run::{self, RunRequest};
use crate::state::StateTables;
use crate::tables::{FederalTables, FilingStatus};

/// The label of a run paid on the regular schedule.
const REGULAR_RUN: &str = "regular";

pub struct ProjectionRequest<'a> {
    pub year: i32,
    pub config: &'a Config,
    pub household: &'a Household,
    pub federal: &'a FederalTables,
    pub state: &'a StateTables,
    pub ledger: &'a Ledger,
    pub tables: &'a ReturnTables,
    pub today: NaiveDate,
}

/// This payroll's part of the year: recorded runs plus projected ones.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct PayrollYear {
    pub recorded_runs: u32,
    pub remaining_periods: u32,
    pub gross: Cents,
    pub w2_box1: Cents,
    pub medicare_wages: Cents,
    pub pretax_401k: Cents,
    pub roth_401k: Cents,
    pub employer_401k: Cents,
    /// Federal income tax and Additional Medicare Tax withheld.
    pub federal_withholding: Cents,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct Retirement {
    pub deferral_ceiling: Cents,
    pub deferred_to_date: Cents,
    /// Deferrals for the year once the remaining periods run as elected.
    pub projected_deferral: Cents,
    /// Ceiling left unused after the projected periods.
    pub room_after_projection: Cents,
    /// Deferral per remaining period that reaches the ceiling, when a period
    /// remains.
    pub per_period_to_fill: Option<Cents>,
    /// Largest employer contribution for the year: 25% of compensation, inside
    /// the section 415(c) limit after the deferrals.
    pub employer_maximum: Cents,
    pub employer_projected: Cents,
    /// Federal tax the projected pre-tax deferrals save against the same
    /// amounts deferred as Roth.
    pub pretax_saving: Cents,
    /// Further federal tax saved if the unused room is deferred pre-tax.
    pub fill_room_saving: Cents,
}

#[derive(Clone, Debug, Serialize)]
pub struct Projection {
    pub year: i32,
    pub filing_status: FilingStatus,
    pub payroll: PayrollYear,
    /// The return's income lines; `wages` includes a spouse's.
    pub income: ReturnIncome,
    pub tax: TaxComputation,
    pub other_withholding: Cents,
    pub estimated: EstimatedTax,
    pub retirement: Retirement,
    pub ira_employee: IraEligibility,
    /// IRA modified adjusted gross income after adding back the specified
    /// adjustments. This can differ from Form 1040 adjusted gross income.
    pub ira_magi: Cents,
    /// On a joint return only.
    pub ira_spouse: Option<IraEligibility>,
}

pub fn project(req: &ProjectionRequest<'_>) -> Result<Projection> {
    let cfg = req.config;
    let h = req.household;
    let year = req.year;

    // The ledger up to the end of the year, completed with projected runs.
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
    let actual = ledger.ytd(year);
    let year_end = NaiveDate::from_ymd_opt(year, 12, 31).expect("December 31 exists");
    for k in 0..remaining {
        let pay_date = year_end - Days::new(u64::from(remaining - 1 - k));
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
    let y = ledger.ytd(year);
    let payroll = PayrollYear {
        recorded_runs: recorded,
        remaining_periods: remaining,
        gross: y.gross,
        w2_box1: y.w2_box1(),
        medicare_wages: y.fica_wages,
        pretax_401k: y.pretax_401k,
        roth_401k: y.roth_401k,
        employer_401k: y.employer_401k,
        federal_withholding: y.federal_income_tax + y.additional_medicare,
    };

    // The return.
    let income = ReturnIncome {
        status: h.filing_status,
        wages: payroll.w2_box1 + h.spouse_wages,
        medicare_wages: payroll.medicare_wages + h.spouse_medicare_wages.unwrap_or_default(),
        other_income: h.other_income,
        interest_and_ordinary_dividends: h.interest_and_ordinary_dividends,
        qualified_dividends: h.qualified_dividends,
        short_term_capital_gains: h.short_term_capital_gains,
        long_term_capital_gains: h.long_term_capital_gains,
        adjustments: h.adjustments,
        itemized_deductions: h.itemized_deductions,
        aged_or_blind_boxes: h.aged_or_blind_boxes,
        credits: h.credits,
    };
    let it = &req.tables.individual;
    let tax = compute_tax(it, &income);

    // Estimated tax.
    let last_installment = estimated_tax_due(year, ESTIMATED_TAX_INSTALLMENTS);
    for p in &h.estimated_payments {
        if p.paid_on.year() < year || p.paid_on > last_installment {
            bail!(
                "household.estimated_payment on {} is outside the {year} payment window, which \
                 runs from January 1 to the last installment's due date.",
                p.paid_on
            );
        }
    }
    let withholding = payroll.federal_withholding + h.other_withholding;
    let estimated = estimated_tax(&EstimatedInput {
        rules: &it.estimated_tax,
        status: h.filing_status,
        year,
        total_tax: tax.total_tax,
        withholding,
        payments: &h.estimated_payments,
        prior_year: h.prior_year_tax.zip(h.prior_year_agi),
        today: req.today,
    });

    // Retirement.
    let ret = &req.federal.retirement;
    let ceiling = ret.deferral_ceiling(cfg.employee.age_this_year);
    let projected_deferral = y.elective_deferral();
    let room_after_projection = (ceiling - projected_deferral).floor_zero();
    let per_period_to_fill = (remaining > 0).then(|| {
        let room = (ceiling - actual.elective_deferral()).floor_zero();
        let n = i64::from(remaining);
        Cents((room.0 + n - 1) / n)
    });
    let compensation = y.gross.min(ret.compensation_limit);
    let counted_deferral = projected_deferral.min(ret.elective_deferral_limit);
    let employer_maximum = compensation
        .mul_ppm(ret.employer_deduction_ppm)
        .min((ret.annual_additions_limit - counted_deferral).floor_zero());
    let with_wages = |wages: Cents| {
        compute_tax(
            it,
            &ReturnIncome {
                wages: wages + h.spouse_wages,
                ..income
            },
        )
        .total_tax
    };
    let retirement = Retirement {
        deferral_ceiling: ceiling,
        deferred_to_date: actual.elective_deferral(),
        projected_deferral,
        room_after_projection,
        per_period_to_fill,
        employer_maximum,
        employer_projected: y.employer_401k,
        pretax_saving: with_wages(payroll.w2_box1 + y.pretax_401k) - tax.total_tax,
        fill_room_saving: tax.total_tax
            - with_wages((payroll.w2_box1 - room_after_projection).floor_zero()),
    };

    // IRAs. An employee with any contribution to the plan this year is an
    // active participant in it.
    let employee_covered = (y.elective_deferral() + y.employer_401k).is_positive();
    let joint = h.filing_status == FilingStatus::MarriedFilingJointly;
    let magi = tax.adjusted_gross_income + h.ira_magi_addbacks;
    let couple = payroll.w2_box1 + h.spouse_wages;
    let ira_employee = eligibility(
        &req.tables.ira,
        h.filing_status,
        magi,
        &IraPerson {
            age_this_year: cfg.employee.age_this_year,
            covered_by_plan: employee_covered,
            spouse_covered_by_plan: h.spouse_covered_by_plan,
            compensation: if joint { couple } else { payroll.w2_box1 },
        },
    );
    let ira_spouse = joint.then(|| {
        eligibility(
            &req.tables.ira,
            h.filing_status,
            magi,
            &IraPerson {
                age_this_year: h.spouse_age_this_year,
                covered_by_plan: h.spouse_covered_by_plan,
                spouse_covered_by_plan: employee_covered,
                compensation: couple,
            },
        )
    });

    Ok(Projection {
        year,
        filing_status: h.filing_status,
        payroll,
        income,
        tax,
        other_withholding: h.other_withholding,
        estimated,
        retirement,
        ira_magi: magi,
        ira_employee,
        ira_spouse,
    })
}
