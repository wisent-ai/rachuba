//! Federal tax computation for one pay period.
//!
//! Income tax withholding follows Worksheet 1A, "Employer's Withholding
//! Worksheet for Percentage Method Tables for Automated Payroll Systems", from
//! Publication 15-T. The line numbers in the code are that worksheet's line
//! numbers, so the calculation can be checked against the published form line
//! by line.
//!
//! The wage bases matter here: Social Security stops at its annual base, the
//! Additional Medicare Tax starts at its threshold, and FUTA stops at $7,000.
//! All three are year-to-date tests, which is why every entry point takes the
//! year-to-date figures alongside this period's wages.

use crate::config::{PayFrequency, W4};
use crate::money::Cents;
use crate::tables::FederalTables;

/// Everything the federal computation needs about one pay period.
#[derive(Clone, Copy, Debug)]
pub struct FederalInput<'a> {
    pub tables: &'a FederalTables,
    pub w4: &'a W4,
    pub frequency: PayFrequency,
    /// Wages subject to income tax withholding: gross less pre-tax deferrals
    /// and any other pre-tax reduction. Worksheet 1A line 1a.
    pub fit_wages: Cents,
    /// Wages subject to Social Security and Medicare. Elective deferrals,
    /// pre-tax or Roth, do NOT reduce this; that is the single most common way
    /// a hand-rolled payroll goes wrong.
    pub fica_wages: Cents,
    /// Wages subject to FUTA. Equal to `fica_wages` in the ordinary case.
    pub futa_wages: Cents,
    pub ytd_ss_wages: Cents,
    pub ytd_medicare_wages: Cents,
    pub ytd_futa_wages: Cents,
    /// FUTA credit reduction for the state, in parts-per-million.
    pub futa_credit_reduction_ppm: i64,
    /// Round withheld income tax to a whole dollar.
    ///
    /// Publication 15-T permits this but does not require it: "you may round
    /// the tax for the pay period to the nearest dollar ... If rounding is
    /// used, it must be used consistently." Commercial payroll providers keep
    /// the cents, and so does this engine by default, because rounding every
    /// period drifts the year-to-date total away from the annual figure the
    /// employee reconciles against on their return.
    pub round_to_dollar: bool,
}

/// Federal taxes for one pay period, split by who bears them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FederalResult {
    pub income_tax: Cents,
    pub ss_employee: Cents,
    pub ss_employer: Cents,
    pub medicare_employee: Cents,
    pub medicare_employer: Cents,
    /// Additional Medicare Tax. Employee only: there is no employer match.
    pub additional_medicare: Cents,
    pub futa_employer: Cents,
    /// Wages that actually attracted Social Security tax this period, after the
    /// annual wage base was applied. Carried into the year-to-date ledger.
    pub ss_taxable_wages: Cents,
    pub futa_taxable_wages: Cents,
}

impl FederalResult {
    /// What the employee has withheld from this paycheck.
    pub fn employee_total(&self) -> Cents {
        self.income_tax + self.ss_employee + self.medicare_employee + self.additional_medicare
    }

    /// What the company owes on top of wages.
    pub fn employer_total(&self) -> Cents {
        self.ss_employer + self.medicare_employer + self.futa_employer
    }

    /// The Form 941 deposit: withheld income tax plus both halves of FICA.
    /// FUTA is deposited separately, against Form 940.
    pub fn form_941_liability(&self) -> Cents {
        self.income_tax
            + self.ss_employee
            + self.ss_employer
            + self.medicare_employee
            + self.medicare_employer
            + self.additional_medicare
    }
}

/// Intermediate values of Worksheet 1A, kept so `rachuba explain` can print the
/// worksheet as the IRS lays it out.
#[derive(Clone, Copy, Debug, Default, serde::Serialize)]
pub struct Worksheet1A {
    pub line_1a: Cents,
    pub line_1b: i64,
    pub line_1c: Cents,
    pub line_1d: Cents,
    pub line_1e: Cents,
    pub line_1f: Cents,
    pub line_1g: Cents,
    pub line_1h: Cents,
    /// Adjusted Annual Wage Amount.
    pub line_1i: Cents,
    pub line_2b: Cents,
    pub line_2c: Cents,
    pub line_2d_ppm: i64,
    pub line_2e: Cents,
    pub line_2f: Cents,
    pub line_2g: Cents,
    /// Tentative Withholding Amount.
    pub line_2h: Cents,
    pub line_3a: Cents,
    pub line_3b: Cents,
    pub line_3c: Cents,
    pub line_4a: Cents,
    /// Amount to withhold this pay period.
    pub line_4b: Cents,
}

/// Run Worksheet 1A for an employee with a 2020-or-later Form W-4.
///
/// The pre-2020 path (worksheet lines 1j through 1l, withholding allowances) is
/// not implemented: it applies only to a Form W-4 signed before 2020 and left
/// unchanged since, which cannot arise for an employee hired by a company that
/// starts running payroll now. Publication 15-T's computational bridge converts
/// such a form to the modern one if the situation ever appears.
pub fn worksheet_1a(input: &FederalInput<'_>) -> Worksheet1A {
    let w = &input.tables.withholding;
    let w4 = input.w4;
    let periods = input.frequency.periods_per_year();

    let mut s = Worksheet1A {
        line_1a: input.fit_wages,
        line_1b: periods,
        ..Default::default()
    };

    // Step 1: annualize, then adjust by the Form W-4 entries.
    s.line_1c = Cents(s.line_1a.0 * periods);
    s.line_1d = w4.step4a_other_income;
    s.line_1e = s.line_1c + s.line_1d;
    s.line_1f = w4.step4b_deductions;
    s.line_1g = w.line_1g_deduction(w4.filing_status, w4.step2_checkbox);
    s.line_1h = s.line_1f + s.line_1g;
    s.line_1i = (s.line_1e - s.line_1h).floor_zero();

    // Step 2: look the Adjusted Annual Wage Amount up in the rate schedule.
    let schedule = w.schedule(w4.filing_status, w4.step2_checkbox);
    let bracket = schedule.bracket_for(s.line_1i);
    s.line_2b = bracket.at_least;
    s.line_2c = bracket.base_tax;
    s.line_2d_ppm = bracket.rate_ppm;
    s.line_2e = s.line_1i - s.line_2b;
    s.line_2f = s.line_2e.mul_ppm(bracket.rate_ppm);
    s.line_2g = s.line_2c + s.line_2f;
    s.line_2h = s.line_2g.div_round(periods);

    // Step 3: spread the Form W-4 Step 3 credits across the year.
    s.line_3a = w4.step3_credits;
    s.line_3b = s.line_3a.div_round(periods);
    s.line_3c = (s.line_2h - s.line_3b).floor_zero();

    // Step 4: add the employee's requested extra withholding.
    //
    // Publication 15-T permits rounding withheld tax to the nearest whole
    // dollar provided it is done consistently. Rounding here, once, at the end,
    // keeps every intermediate line exact.
    s.line_4a = w4.step4c_extra_withholding;
    let total = s.line_3c + s.line_4a;
    s.line_4b = if input.round_to_dollar {
        total.round_to_dollar()
    } else {
        total
    };

    s
}

/// Compute every federal tax for one pay period.
pub fn compute(input: &FederalInput<'_>) -> FederalResult {
    let t = input.tables;

    // Social Security stops once year-to-date wages reach the annual base.
    let ss_remaining = (t.social_security.wage_base - input.ytd_ss_wages).floor_zero();
    let ss_taxable = input.fica_wages.min(ss_remaining);

    // Medicare has no wage base. The Additional Medicare Tax applies to the
    // part of this period's wages that carries year-to-date wages past the
    // threshold, and the employer does not match it.
    let addl_threshold = t.medicare.additional_threshold;
    let ytd_after = input.ytd_medicare_wages + input.fica_wages;
    let addl_taxable = if ytd_after > addl_threshold {
        let over = ytd_after - addl_threshold;
        over.min(input.fica_wages)
    } else {
        Cents::ZERO
    };

    // FUTA stops at its own, much lower, wage base.
    let futa_remaining = (t.futa.wage_base - input.ytd_futa_wages).floor_zero();
    let futa_taxable = input.futa_wages.min(futa_remaining);
    let futa_rate = t.futa.net_rate_ppm() + input.futa_credit_reduction_ppm;

    FederalResult {
        income_tax: worksheet_1a(input).line_4b,
        ss_employee: ss_taxable.mul_ppm(t.social_security.employee_rate_ppm),
        ss_employer: ss_taxable.mul_ppm(t.social_security.employer_rate_ppm),
        medicare_employee: input.fica_wages.mul_ppm(t.medicare.employee_rate_ppm),
        medicare_employer: input.fica_wages.mul_ppm(t.medicare.employer_rate_ppm),
        additional_medicare: addl_taxable.mul_ppm(t.medicare.additional_rate_ppm),
        futa_employer: futa_taxable.mul_ppm(futa_rate),
        ss_taxable_wages: ss_taxable,
        futa_taxable_wages: futa_taxable,
    }
}




