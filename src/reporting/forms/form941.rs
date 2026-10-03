//! Employer's quarterly federal tax return, derived from the ledger.

use super::Line;
use crate::ledger::Ledger;
use crate::money::Cents;
use crate::tables::FederalTables;

/// Employer's Quarterly Federal Tax Return.
#[derive(serde::Serialize)]
pub struct Form941 {
    pub year: i32,
    pub quarter: u32,
    pub employees: u32,
    pub lines: Vec<Line>,
    /// Total deposited during the quarter, taken from the ledger's deposit
    /// records. The difference against total tax is the balance due.
    pub deposits: Cents,
    pub balance_due: Cents,
    pub overpayment: Cents,
}

impl Form941 {
    pub fn build(ledger: &Ledger, tables: &FederalTables, year: i32, quarter: u32) -> Form941 {
        let q = ledger.quarter_totals(year, quarter);

        // Lines 5a and 5c carry the combined employee and employer rate: 12.4%
        // for Social Security and 2.9% for Medicare. Line 5d is the
        // employee-only Additional Medicare Tax at 0.9%.
        let ss_combined = q.ss_employee + q.ss_employer;
        let medicare_combined = q.medicare_employee + q.medicare_employer;
        let line_5e = ss_combined + medicare_combined + q.additional_medicare;
        let line_6 = q.federal_income_tax + line_5e;

        // The additional-tax wage base is not accumulated directly, so it is
        // recovered from the tax at the statutory rate.
        let addl_wages = if tables.medicare.additional_rate_ppm == 0 {
            Cents::ZERO
        } else {
            Cents(
                (q.additional_medicare.0 as i128 * crate::money::PPM
                    / tables.medicare.additional_rate_ppm as i128) as i64,
            )
        };

        let deposits: Cents = ledger
            .runs_in_quarter(year, quarter)
            .filter(|r| r.taxes_deposited.is_some())
            .map(|r| r.form_941_liability())
            .sum();

        let employees = if q.runs > 0 { 1 } else { 0 };

        Form941 {
            year,
            quarter,
            employees,
            lines: vec![
                Line {
                    number: "2",
                    label: "Wages, tips, and other compensation",
                    value: q.fit_wages,
                },
                Line {
                    number: "3",
                    label: "Federal income tax withheld",
                    value: q.federal_income_tax,
                },
                Line {
                    number: "5a",
                    label: "Taxable social security wages",
                    value: q.ss_wages,
                },
                Line {
                    number: "5a",
                    label: "  Social security tax (12.4%)",
                    value: ss_combined,
                },
                Line {
                    number: "5c",
                    label: "Taxable Medicare wages and tips",
                    value: q.fica_wages,
                },
                Line {
                    number: "5c",
                    label: "  Medicare tax (2.9%)",
                    value: medicare_combined,
                },
                Line {
                    number: "5d",
                    label: "Wages subject to Additional Medicare Tax",
                    value: addl_wages,
                },
                Line {
                    number: "5d",
                    label: "  Additional Medicare Tax (0.9%)",
                    value: q.additional_medicare,
                },
                Line {
                    number: "5e",
                    label: "Total social security and Medicare taxes",
                    value: line_5e,
                },
                Line {
                    number: "6",
                    label: "Total taxes before adjustments",
                    value: line_6,
                },
                Line {
                    number: "10",
                    label: "Total taxes after adjustments",
                    value: line_6,
                },
                Line {
                    number: "12",
                    label: "Total taxes after adjustments and credits",
                    value: line_6,
                },
                Line {
                    number: "13",
                    label: "Total deposits for this quarter",
                    value: deposits,
                },
            ],
            deposits,
            balance_due: (line_6 - deposits).floor_zero(),
            overpayment: (deposits - line_6).floor_zero(),
        }
    }
}
