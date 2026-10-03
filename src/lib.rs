//! rachuba: payroll computation for a company with one employee, and the
//! projection of that employee's own federal return.
//!
//! The engine is a library so the arithmetic can be used without the command
//! line: the binary in `main.rs` is a thin front end over these modules.
//!
//! The shape of a payroll run is fixed and documented in [`run::compute`].
//! Everything else here supports it:
//!
//! * [`money`] — exact integer-cent arithmetic. No float touches a dollar.
//! * [`tables`] — per-year federal tax tables, loaded from data files.
//! * [`config`] — the payer, the employee, and their elections.
//! * [`federal`] — Worksheet 1A, FICA and FUTA.
//! * [`state`], [`state_ca`], [`state_ny`] — state and local withholding.
//! * [`ledger`] — committed runs and the year-to-date totals derived from them.
//! * [`calendar`] — deposit and filing due dates.
//! * [`forms`] — Form 941, Form 940 and W-2 line values.
//! * [`stub`] — the wage statement.
//! * [`personal`] — the employee's own Form 1040: tax, estimated tax, 401(k)
//!   room and IRA eligibility for the year.
//!
//! Rolling to a new tax year is a data change: add `tables/federal-<year>/`
//! and `tables/state-<code>-<year>/` section folders transcribed from that year's
//! publications. No code change is required for a rate or threshold movement.

mod reporting;
pub use reporting::{calendar, forms, stub};
mod records;
pub use records::{config, ledger};
mod calculation;
pub use calculation::{federal, money, personal, run, state, state_ca, state_ny, tables};
