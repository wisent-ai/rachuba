//! The legal holidays a payroll's due dates move around, read from the year's
//! table instead of computed.
//!
//! Which days are holidays is law the Congress, the President and the District
//! of Columbia change: Juneteenth joined 5 U.S.C. 6103(a) in 2021, District of
//! Columbia Emancipation Day began in 2007, and Inauguration Day comes every
//! fourth year with its own observance rule. Each year's observed dates are
//! therefore transcribed from the published schedules into
//! `tables/federal-<year>/legal-holidays.toml`, cited there, and a date the
//! table does not cover is refused by name rather than read as a working day.

use anyhow::{bail, Result};
use chrono::{Datelike, NaiveDate, Weekday};
use serde::Deserialize;

use super::add_days;

/// One holiday as it is observed: the day offices close, after any weekend
/// shift the authority applied.
#[derive(Clone, Debug, Deserialize)]
pub struct Holiday {
    pub name: String,
    pub observed: NaiveDate,
}

/// The legal holidays observed between two dates, as the year's table declares
/// them.
#[derive(Clone, Debug, Deserialize)]
pub struct LegalHolidays {
    /// The authorities the dates were transcribed from.
    pub source: String,
    /// The first day the table answers for.
    pub covers_from: NaiveDate,
    /// The last day the table answers for.
    pub covers_through: NaiveDate,
    /// Days designated as holidays by the Federal Government, as observed: the
    /// set behind the Department of Labor business day, 29 CFR 2510.3-102(e).
    pub federal: Vec<Holiday>,
    /// The District of Columbia's own legal holidays beyond the federal ones
    /// (Emancipation Day, Inauguration Day), which 26 U.S.C. 7503 adds for
    /// every IRS deposit and filing date.
    pub district_of_columbia: Vec<Holiday>,
}

impl LegalHolidays {
    /// Refuses a table that cannot be right: a reversed coverage, a holiday
    /// outside it, or a holiday observed on a Saturday or Sunday, which no
    /// observance rule produces and which is therefore a transcription error.
    pub fn validate(&self) -> Result<()> {
        if self.covers_from > self.covers_through {
            bail!(
                "legal_holidays.covers_from {} is after legal_holidays.covers_through {}",
                self.covers_from,
                self.covers_through
            );
        }
        self.validate_list("legal_holidays.federal", &self.federal)?;
        self.validate_list(
            "legal_holidays.district_of_columbia",
            &self.district_of_columbia,
        )
    }

    fn validate_list(&self, key: &str, holidays: &[Holiday]) -> Result<()> {
        for holiday in holidays {
            if holiday.observed < self.covers_from || holiday.observed > self.covers_through {
                bail!(
                    "{key} lists {} on {}, outside the table's coverage {} through {}",
                    holiday.name,
                    holiday.observed,
                    self.covers_from,
                    self.covers_through
                );
            }
            if is_weekend(holiday.observed) {
                bail!(
                    "{key} lists {} as observed on {}, a {}; an observed holiday is a weekday, \
                     so transcribe the day the published schedule observes it",
                    holiday.name,
                    holiday.observed,
                    holiday.observed.weekday()
                );
            }
        }
        Ok(())
    }

    /// Refuses a date the table does not answer for, naming the coverage and
    /// where the missing year's schedule belongs.
    fn covered(&self, d: NaiveDate) -> Result<()> {
        if d < self.covers_from || d > self.covers_through {
            bail!(
                "{d} is outside the legal holiday table, which covers {} through {} ({}). \
                 Transcribe the published holiday schedules for {} into \
                 tables/federal-<year>/legal-holidays.toml before computing a due date that \
                 depends on that day.",
                self.covers_from,
                self.covers_through,
                self.source,
                d.year()
            );
        }
        Ok(())
    }

    /// True when a federal holiday is observed on `d`.
    pub fn is_federal_holiday(&self, d: NaiveDate) -> Result<bool> {
        self.covered(d)?;
        Ok(self.federal.iter().any(|h| h.observed == d))
    }

    /// True when `d` is a "legal holiday" in the sense 26 U.S.C. 7503 gives
    /// the term: a legal holiday in the District of Columbia, which is the
    /// federal holidays plus the District's own.
    ///
    /// Statewide legal holidays are excluded. 26 CFR 31.6302-1(c)(4): "the
    /// term 'legal holiday' does not include other Statewide legal holidays".
    pub fn is_irs_legal_holiday(&self, d: NaiveDate) -> Result<bool> {
        let federal = self.is_federal_holiday(d)?;
        Ok(federal || self.district_of_columbia.iter().any(|h| h.observed == d))
    }

    /// True when a deposit or a return can be made on `d`, under 26 CFR
    /// 31.6302-1(c)(4): "Business days include every calendar day other than
    /// Saturdays, Sundays, or legal holidays", with "legal holiday" in the
    /// section 7503 sense.
    pub fn is_irs_business_day(&self, d: NaiveDate) -> Result<bool> {
        let holiday = self.is_irs_legal_holiday(d)?;
        Ok(!is_weekend(d) && !holiday)
    }

    /// `d` itself when it is an IRS business day, otherwise the next one.
    ///
    /// 26 U.S.C. 7503: "When the last day prescribed... falls on Saturday,
    /// Sunday, or a legal holiday, the performance of such act shall be
    /// considered timely if it is performed on the next succeeding day which
    /// is not a Saturday, Sunday, or a legal holiday."
    pub fn irs_business_day_on_or_after(&self, d: NaiveDate) -> Result<NaiveDate> {
        let mut candidate = d;
        while !self.is_irs_business_day(candidate)? {
            candidate = add_days(candidate, 1);
        }
        Ok(candidate)
    }
}

pub(super) fn is_weekend(d: NaiveDate) -> bool {
    matches!(d.weekday(), Weekday::Sat | Weekday::Sun)
}
