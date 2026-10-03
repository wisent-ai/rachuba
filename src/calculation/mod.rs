//! Exact payroll computation, federal withholding, state dispatch, and the
//! employee's own federal return.

pub mod federal;
pub mod money;
pub mod personal;
pub mod run;
pub mod state;
pub mod state_ca;
pub mod state_ny;
pub mod tables;
