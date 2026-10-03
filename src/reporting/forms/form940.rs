//! Employer's annual federal unemployment tax return.

use super::Line;
use crate::calendar::QUARTERS_PER_YEAR;
use crate::ledger::Ledger;
use crate::money::Cents;
use crate::tables::FederalTables;

/// Employer's Annual Federal Unemployment (FUTA) Tax Return.
#[derive(serde::Serialize)]
pub struct Form940 {
    pub year: i32,
    pub lines: Vec<Line>,
    /// Quarterly liabilities for Part 5, which is required whenever the annual
    /// FUTA tax exceeds the deposit threshold.
    pub quarterly: [Cents; 4],
    pub part5_required: bool,
}

impl Form940 {
    pub fn build(ledger: &Ledger, tables: &FederalTables, year: i32) -> Form940 {
        let y = ledger.ytd(year);
        // Line 5 is the part of total pay above the wage base, which is what
        // the form asks for rather than the taxable amount directly.
        let over_base = (y.gross - y.futa_wages).floor_zero();

        let mut quarterly = [Cents::ZERO; QUARTERS_PER_YEAR as usize];
        for q in 1..=QUARTERS_PER_YEAR {
            quarterly[(q - 1) as usize] = ledger.quarter_totals(year, q).futa_employer;
        }

        Form940 {
            year,
            lines: vec![
                Line {
                    number: "3",
                    label: "Total payments to all employees",
                    value: y.gross,
                },
                Line {
                    number: "5",
                    label: "Payments exceeding the $7,000 wage base",
                    value: over_base,
                },
                Line {
                    number: "7",
                    label: "Total taxable FUTA wages",
                    value: y.futa_wages,
                },
                Line {
                    number: "8",
                    label: "FUTA tax before adjustments",
                    value: y.futa_employer,
                },
                Line {
                    number: "12",
                    label: "Total FUTA tax after adjustments",
                    value: y.futa_employer,
                },
            ],
            quarterly,
            part5_required: y.futa_employer > tables.futa.deposit_threshold,
        }
    }
}
