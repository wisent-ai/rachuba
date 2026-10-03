//! State and local tax dispatch.
//!
//! Each state that this engine supports needs two things: a data file under the
//! tables directory, and a module that knows the arithmetic that state's
//! publication prescribes. The two cannot be separated, because state
//! withholding methods genuinely differ in shape — New York de-annualizes its
//! rate table and then applies a flat-rate cliff above a threshold, which no
//! amount of generic bracket evaluation reproduces.
//!
//! An unsupported state is therefore a hard error, never a silent zero. A
//! payroll that quietly withholds no state tax is worse than one that refuses
//! to run, because the shortfall only surfaces when the employee files.

use std::path::Path;

use anyhow::{bail, Result};

use crate::config::{Company, Employee};
use crate::money::Cents;
use crate::{state_ca, state_ny};

/// State and local amounts for one pay period.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StateResult {
    pub state_income_tax: Cents,
    /// City or municipal income tax, such as the New York City resident tax or
    /// the Yonkers surcharge.
    pub local_income_tax: Cents,
    /// State disability insurance withheld from the employee.
    pub sdi_employee: Cents,
    /// Paid family leave contribution withheld from the employee.
    pub pfl_employee: Cents,
    /// State unemployment insurance, which the employer alone pays.
    pub sui_employer: Cents,
    /// A further employer-side state levy reported separately from
    /// unemployment: California's Employment Training Tax. Zero where the state
    /// has no such levy.
    pub training_tax_employer: Cents,
}

impl StateResult {
    /// Everything withheld from the employee on the state side.
    pub fn employee_total(&self) -> Cents {
        self.state_income_tax + self.local_income_tax + self.sdi_employee + self.pfl_employee
    }
}

pub struct StateInput<'a> {
    pub tables: &'a StateTables,
    pub employee: &'a Employee,
    pub company: &'a Company,
    /// Wages subject to state income tax. New York conforms to the federal
    /// treatment of elective deferrals, so this is gross less the pre-tax
    /// deferral, the same figure as the federal one.
    pub taxable_wages: Cents,
    /// Gross wages, which is what the insurance contributions and the
    /// unemployment wage base run on.
    pub gross_wages: Cents,
    pub ytd_sdi: Cents,
    pub ytd_pfl: Cents,
    pub ytd_sui_wages: Cents,
}

/// The tables for whichever state the company withholds for.
pub enum StateTables {
    NewYork(Box<state_ny::Tables>),
    California(Box<state_ca::Tables>),
}

impl StateTables {
    pub fn load(dir: &Path, code: &str, year: i32) -> Result<StateTables> {
        let path = dir.join(format!("state-{code}-{year}"));
        match code {
            "NY" => Ok(StateTables::NewYork(Box::new(state_ny::Tables::load(
                &path, year,
            )?))),
            "CA" => Ok(StateTables::California(Box::new(state_ca::Tables::load(
                &path, year,
            )?))),
            other => bail!(
                "no state engine for {other}. Supported: CA, NY. Adding a state needs both a data \
                 folder at {} and a computation module, because withholding methods differ in \
                 shape between states and cannot be derived from a rate table alone.",
                path.display()
            ),
        }
    }

    pub fn source(&self) -> &str {
        match self {
            StateTables::NewYork(t) => &t.source,
            StateTables::California(t) => &t.source,
        }
    }

    /// A caution the state engine wants surfaced before payroll runs, if any.
    pub fn advisory(&self, futa_credit_reduction_ppm: i64) -> Option<String> {
        match self {
            StateTables::NewYork(_) => None,
            StateTables::California(t) => state_ca::futa_advisory(t, futa_credit_reduction_ppm),
        }
    }
}

pub fn compute(input: &StateInput<'_>) -> Result<StateResult> {
    match input.tables {
        StateTables::NewYork(t) => state_ny::compute(t, input),
        StateTables::California(t) => state_ca::compute(t, input),
    }
}
