//! rachuba: payroll for a company with one employee.
//!
//! Computes gross to net from the published federal and state tables, records
//! every run in a ledger under version control, and tells you what to deposit,
//! where, and by when.
//!
//! It does not move money. Deposits go through EFTPS and the state portal, and
//! the elective deferral goes to the plan trustee, all by the operator. What
//! this tool guarantees is that the amounts and the due dates are right.

mod cli;

fn main() -> anyhow::Result<()> {
    cli::main()
}
