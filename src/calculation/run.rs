//! Assembling one payroll run from configuration, tables and history.
//!
//! The order of operations here is the part that is easy to get wrong, so it is
//! stated once, explicitly:
//!
//! 1. Gross wages for the period.
//! 2. Elective deferrals, capped at the room left under section 402(g) for the
//!    year. A payroll that quietly overshoots the limit creates an excess
//!    deferral that must be distributed by 15 April or be taxed twice.
//! 3. Wages subject to federal income tax withholding: gross less the pre-tax
//!    deferral. Roth deferrals do not reduce it.
//! 4. Wages subject to Social Security, Medicare and FUTA: gross, undiminished
//!    by either kind of deferral.
//! 5. Federal taxes, then state and local taxes.
//! 6. Net pay: gross less both deferrals and every employee-side tax.

use anyhow::Result;
use chrono::NaiveDate;

use crate::config::Config;
use crate::federal::{self, FederalInput};
use crate::ledger::{Ledger, PayrollRun};
use crate::money::Cents;
use crate::state::{self, StateInput, StateTables};
use crate::tables::FederalTables;

/// A computed but not yet committed payroll run, with the notes explaining any
/// place the engine departed from the plain election.
#[derive(serde::Serialize)]
pub struct ComputedRun {
    pub run: PayrollRun,
    pub notes: Vec<String>,
}

pub struct RunRequest<'a> {
    pub config: &'a Config,
    pub federal_tables: &'a FederalTables,
    pub state_tables: &'a StateTables,
    pub ledger: &'a Ledger,
    pub pay_date: NaiveDate,
    /// Override the configured gross, for a bonus or a short period.
    pub gross_override: Option<Cents>,
    pub kind: String,
}

pub fn compute(req: &RunRequest<'_>) -> Result<ComputedRun> {
    let cfg = req.config;
    let ft = req.federal_tables;
    let ytd = req.ledger.ytd_before(req.pay_date);
    let mut notes = Vec::new();

    let gross = req.gross_override.unwrap_or(cfg.employee.gross_per_period);

    // Step 2: elective deferrals, capped at the year's remaining room.
    let ceiling = ft.retirement.deferral_ceiling(cfg.employee.age_this_year);
    let room = (ceiling - ytd.elective_deferral()).floor_zero();

    let mut pretax = cfg.retirement_401k.pretax.applied_to(gross);
    let mut roth = cfg.retirement_401k.roth.applied_to(gross);

    // Deferrals can never exceed the paycheck they come out of.
    if pretax + roth > gross {
        notes.push(format!(
            "elected deferrals of {} exceed gross pay of {gross}; reduced to the full paycheck.",
            pretax + roth
        ));
        roth = (gross - pretax).floor_zero();
        pretax = pretax.min(gross);
    }

    if pretax + roth > room {
        let requested = pretax + roth;
        // Fill the remaining room pre-tax first, then Roth, preserving the
        // employee's ordering preference for whichever survives the cap.
        pretax = pretax.min(room);
        roth = (room - pretax).floor_zero();
        notes.push(format!(
            "elective deferral capped at {room}: {requested} was elected but the {} limit of \
             {ceiling} leaves only that much room.",
            ft.year
        ));
    }

    // Employer contribution, held under both the 25%-of-compensation limit and
    // the section 415(c) annual additions cap.
    let mut employer = cfg.retirement_401k.employer.applied_to(gross);
    let comp_cap = (ytd.gross + gross).mul_ppm(250_000) - ytd.employer_401k;
    if employer > comp_cap.floor_zero() {
        notes.push(format!(
            "employer contribution reduced from {employer} to {} by the 25%-of-compensation limit.",
            comp_cap.floor_zero()
        ));
        employer = comp_cap.floor_zero();
    }
    let additions_room =
        (ft.retirement.annual_additions_limit - ytd.annual_additions() - pretax - roth)
            .floor_zero();
    if employer > additions_room {
        notes.push(format!(
            "employer contribution reduced from {employer} to {additions_room} by the section \
             415(c) limit of {}.",
            ft.retirement.annual_additions_limit
        ));
        employer = additions_room;
    }

    // Steps 3 and 4: the two wage figures that drive everything downstream.
    let fit_wages = (gross - pretax).floor_zero();
    let fica_wages = gross;

    // Step 5: federal, then state.
    let fed = federal::compute(&FederalInput {
        tables: ft,
        w4: &cfg.employee.w4,
        frequency: cfg.employee.pay_frequency,
        fit_wages,
        fica_wages,
        futa_wages: fica_wages,
        ytd_ss_wages: ytd.ss_wages,
        ytd_medicare_wages: ytd.fica_wages,
        ytd_futa_wages: ytd.futa_wages,
        futa_credit_reduction_ppm: cfg.company.futa_credit_reduction_ppm,
        round_to_dollar: cfg.company.round_withholding_to_dollar,
    });

    let st = state::compute(&StateInput {
        tables: req.state_tables,
        employee: &cfg.employee,
        company: &cfg.company,
        taxable_wages: fit_wages,
        gross_wages: gross,
        ytd_sdi: ytd.sdi_employee,
        ytd_pfl: ytd.pfl_employee,
        ytd_sui_wages: ytd.gross,
    })?;

    // Step 6: net pay.
    let employee_withholding = fed.employee_total() + st.employee_total();
    let net_pay = gross - pretax - roth - employee_withholding;

    if net_pay < Cents::ZERO {
        notes.push(format!(
            "net pay is negative ({net_pay}): deferrals plus withholding exceed gross wages. \
             Reduce the deferral election before paying this period."
        ));
    }

    Ok(ComputedRun {
        run: PayrollRun {
            pay_date: req.pay_date,
            kind: req.kind.clone(),
            gross,
            pretax_401k: pretax,
            roth_401k: roth,
            employer_401k: employer,
            fit_wages,
            fica_wages,
            ss_taxable_wages: fed.ss_taxable_wages,
            futa_taxable_wages: fed.futa_taxable_wages,
            federal_income_tax: fed.income_tax,
            ss_employee: fed.ss_employee,
            ss_employer: fed.ss_employer,
            medicare_employee: fed.medicare_employee,
            medicare_employer: fed.medicare_employer,
            additional_medicare: fed.additional_medicare,
            futa_employer: fed.futa_employer,
            state_income_tax: st.state_income_tax,
            local_income_tax: st.local_income_tax,
            sdi_employee: st.sdi_employee,
            pfl_employee: st.pfl_employee,
            sui_employer: st.sui_employer,
            state_training_tax: st.training_tax_employer,
            net_pay,
            deferral_remitted: None,
            taxes_deposited: None,
        },
        notes,
    })
}
