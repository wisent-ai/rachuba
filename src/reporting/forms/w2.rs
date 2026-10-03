//! Annual wage and tax statement line values.

use super::Line;
use crate::ledger::Ledger;
use crate::money::Cents;
use crate::tables::FederalTables;

/// Wage and Tax Statement, boxed as the SSA online form presents it.
#[derive(serde::Serialize)]
pub struct FormW2 {
    pub year: i32,
    pub boxes: Vec<Line>,
    pub box12: Vec<(&'static str, &'static str, Cents)>,
    /// Box 13 "Retirement plan" is checked for any employee who was an active
    /// participant during the year.
    pub retirement_plan_checked: bool,
    pub state_code: String,
    pub locality: Option<String>,
}

impl FormW2 {
    pub fn build(
        ledger: &Ledger,
        tables: &FederalTables,
        year: i32,
        state_code: &str,
        locality: Option<String>,
    ) -> FormW2 {
        let y = ledger.ytd(year);

        // Box 3 is capped at the Social Security wage base; box 5 never is.
        let box3 = y.ss_wages.min(tables.social_security.wage_base);
        // Box 6 carries the Additional Medicare Tax along with regular
        // Medicare withholding.
        let box6 = y.medicare_employee + y.additional_medicare;

        let mut box12 = Vec::new();
        if y.pretax_401k.is_positive() {
            box12.push((
                "D",
                "Elective deferrals to a section 401(k) plan",
                y.pretax_401k,
            ));
        }
        if y.roth_401k.is_positive() {
            box12.push((
                "AA",
                "Designated Roth contributions under a 401(k) plan",
                y.roth_401k,
            ));
        }

        let mut boxes = vec![
            Line {
                number: "1",
                label: "Wages, tips, other compensation",
                value: y.w2_box1(),
            },
            Line {
                number: "2",
                label: "Federal income tax withheld",
                value: y.federal_income_tax,
            },
            Line {
                number: "3",
                label: "Social security wages",
                value: box3,
            },
            Line {
                number: "4",
                label: "Social security tax withheld",
                value: y.ss_employee,
            },
            Line {
                number: "5",
                label: "Medicare wages and tips",
                value: y.fica_wages,
            },
            Line {
                number: "6",
                label: "Medicare tax withheld",
                value: box6,
            },
        ];
        if y.state_employee_insurance().is_positive() {
            boxes.push(Line {
                number: "14",
                label: "Other: state disability and paid family leave",
                value: y.state_employee_insurance(),
            });
        }
        boxes.push(Line {
            number: "16",
            label: "State wages, tips, etc.",
            value: y.fit_wages,
        });
        boxes.push(Line {
            number: "17",
            label: "State income tax",
            value: y.state_income_tax,
        });
        if y.local_income_tax.is_positive() {
            boxes.push(Line {
                number: "18",
                label: "Local wages, tips, etc.",
                value: y.fit_wages,
            });
            boxes.push(Line {
                number: "19",
                label: "Local income tax",
                value: y.local_income_tax,
            });
        }

        FormW2 {
            year,
            boxes,
            box12,
            retirement_plan_checked: y.annual_additions().is_positive(),
            state_code: state_code.to_string(),
            locality,
        }
    }
}
