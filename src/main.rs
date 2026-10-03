//! rachuba: payroll for a company with one employee.
//!
//! Computes gross to net from the published federal and state tables, records
//! every run in a ledger under version control, and tells you what to deposit,
//! where, and by when.
//!
//! It does not move money. Deposits go through EFTPS and the state portal, and
//! elective deferrals go to the plan trustee. Dates and amounts depend on the
//! published tables and company entries; undetermined rates remain warnings.

mod cli;

fn main() -> anyhow::Result<()> {
    cli::main()
}
