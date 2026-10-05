# rachuba

Payroll for a company with one employee. Computes gross-to-net from published
federal and supported state tables, records paid runs in a ledger under version
control, and computes federal employment tax deposit and 401(k) remittance
deadlines. It also projects the employee's own federal return for the year, so
the estimated tax still owed and the worth of each retirement choice are known
before the year closes.

It does not move money. The operator deposits federal employment taxes through
EFTPS and sends elective deferrals to the plan trustee; `deposited` and
`remitted` record those payments. The calendar shows federal payroll form due
dates, but the ledger does not record filings. State payroll deposits and
returns, company income and franchise taxes, annual reports, and information
returns such as 1099s remain outside this tracking.

The documentation source has one page per command in `rachuba-landing`. Its
public address is <https://rachuba.wisent.com/docs/> when the site is published.

## Why this exists

For a single-employee C-corp, a payroll SaaS subscription buys three things:
arithmetic, due-date tracking, and someone else's liability. The arithmetic is
two hundred lines. The due-date tracking is a calendar. Only the third is worth
paying for, and only if you would otherwise miss a deposit.

The real maintenance cost is not the code. It is that Publication 15-T reissues
every December and each state reissues its own tables alongside it. That cost is
isolated here: tax tables are data files, one per jurisdiction per year, and
rolling the engine forward is a transcription job with no code change.

## Status

Federal withholding is complete for any state. State withholding is implemented
for **California** and **New York**.

California covers Method B exact calculation for all three filing statuses and
all eight payroll periods, SDI, Unemployment Insurance and the Employment
Training Tax, and reproduces all four of DE 44's worked Method B examples to
the cent.

New York covers the Exact Calculation Method with its Method III top-rate
regime, New York City resident tax, the Yonkers resident surcharge, the Yonkers
nonresident earnings tax, DBL and Paid Family Leave.

An unsupported state is a hard error, never a silent zero.

The return projection is federal: Form 1040 tax with the capital gains
worksheet, the net investment income tax, the Additional Medicare Tax, the
estimated tax safe harbor, 401(k) room and IRA eligibility. State income tax
returns are not projected.

## Install

```
cargo build --release
```

## Use

```
rachuba init                              # write a starting rachuba.toml
rachuba explain                           # Worksheet 1A, line by line
rachuba run --pay-date 2026-09-30         # compute, record nothing
rachuba run --pay-date 2026-09-30 --commit
rachuba stub --pay-date 2026-09-30        # reprint a wage statement
rachuba ytd                               # year-to-date totals
rachuba check                             # deferral and section 415(c) limits
rachuba owed                              # money that has not moved yet
rachuba calendar                          # supported federal payroll and 401(k) due dates
rachuba deposited --pay-date 2026-09-30   # record an EFTPS deposit
rachuba remitted  --pay-date 2026-09-30   # record a deferral reaching the plan
rachuba retract --pay-date 2026-09-30 --record deposit   # take back a deposited/remitted record
rachuba void --pay-date 2026-09-30        # take back the last committed run
rachuba form 941 --quarter 3
rachuba form 940
rachuba form w2
rachuba return                            # the employee's own federal return
```

`rachuba run` without `--commit` prints the stub and changes nothing. Nothing
enters the ledger until you ask for it. `rachuba void` takes the last committed
run back out; it refuses an earlier run, because every later run was computed
against its year-to-date totals, and a run whose taxes were deposited or whose
deferral was remitted, until `rachuba retract --record deposit|remittance`
takes back that record. A retracted payment is reported by `owed` as due again.

Every command prints text for a person; `--json`, anywhere on the line, prints
the same figures as JSON for a machine — the run, the year-to-date totals, the
findings, the due dates, the form values, each amount as a `"1234.56"` string so
no float rounds a dollar. `owed` and `check` still exit 1 after printing when a
due date has passed or a limit is breached; a wrong invocation exits 2.

## The monthly routine

1. `rachuba run --pay-date <date> --commit`
2. Pay the employee the net figure.
3. Transfer the elective deferral to the plan trustee, then
   `rachuba remitted --pay-date <date>`. The Department of Labor safe harbor at
   29 CFR 2510.3-102(a)(2) is seven business days for a plan with fewer than 100
   participants; past that the money is a prohibited transaction and correction
   runs through the DOL Voluntary Fiduciary Correction Program.
4. Deposit the Form 941 taxes through EFTPS, then
   `rachuba deposited --pay-date <date>`.
5. Commit the ledger.

`rachuba owed` lists anything from steps 3 and 4 that has not happened, and
exits non-zero once a due date has passed.

## The employee's own return

`rachuba return [--year <year>]` projects the employee's Form 1040 for the
year from the ledger, the remaining pay periods computed as a real run would
be, and the `[household]` section of the configuration: tax with the capital
gains worksheet, the net investment income tax and the Additional Medicare
Tax; each estimated tax installment's shortfall; 401(k) room, the employer
maximum and the tax each saves; IRA deduction and Roth eligibility. It records
nothing. Every figure it prints and every refusal is on
<https://rachuba.wisent.com/docs/cli/return/>.

## California, specifically

Four things about California that a generic bracket evaluator gets wrong:

**Form DE 4 carries two allowance counts that never mix.** Line 1 is regular
allowances and drives the Table 1 and Table 3 column plus the Table 4 credit.
Line 2 is additional allowances for estimated deductions and drives Table 2 and
nothing else. DE 44 page 43 footnote 1 says estimated-deduction allowances "must
not be used in the determination of tax credits to be subtracted", and DE 44's
own Example B is the case that separates them.

**The low income exemption is a short circuit, not a deduction.** Gross at or
below the Table 1 figure withholds nothing at all. One cent above it, the full
computation runs on the entire gross and the exemption is subtracted from
nothing. The cliff is published and is not smoothed.

**Tables 1 and 3 are not keyed by filing status.** Their married column splits
on the regular allowance count, while the rate table is chosen by marital status
alone. A married employee with one allowance therefore takes single-sized
deduction figures and the married rate table.

**SDI has no wage ceiling.** SB 951 removed it effective 2024, so the 2026 rate
of 1.3% applies to every dollar with no maximum withholding. An engine carrying
a stale cap under-withholds all year. California Paid Family Leave is funded
from the same contribution and is not a separate deduction.

Two more things that are not code but cost money. California bars local income
tax statewide under Revenue and Taxation Code section 17041.5, and San
Francisco's Payroll Expense Tax was repealed effective 2021, so there is no San
Francisco employee withholding at all; the city's Gross Receipts Tax is an
employer business tax outside this engine. California has been a FUTA credit
reduction state every year since 2022, determined at 1.2% for 2025. The 2026
rate is not determined until 10 November 2026 and then applies retroactively
to the whole year. The example leaves it at zero rather than guessing a rate.
For California, `rachuba run` warns while the rate is undetermined, even when
an estimated accrual is entered. Once Schedule A (Form 940) determines it, a
different configured rate still warns; update the value and revisit the year's
liability.

## Two withholding defaults

**Withheld income tax is not rounded to the dollar.** Publication 15-T permits
rounding and does not require it. Payroll providers keep the cents, and
rounding every period drifts the year-to-date total away from the annual figure
the employee reconciles against. `round_withholding_to_dollar` turns it on if
a particular employer wants it.

**California withholding uses the annualized method by default.** DE 44
sanctions two paths that disagree by a few cents, because the per-period tables
are rounded independently of the annual one. The direct path reads the printed
table for the payroll period; the annualized path of DE 44 page 45 multiplies
wages up to a year, reads every table at annual, and divides at the end. Twelve
annualized payrolls sum to exactly the annual liability where twelve direct ones
do not, and the annualized path is what providers compute, so adopting this
engine mid-year leaves no step in the year-to-date figures.
`state_annualized_method` selects it. DE 44's own worked examples cover both:
A through D on the direct path, E and F on the annualized one.

Whichever is chosen must be held for the whole year. Changing it mid-year is
what breaks reconciliation, not the choice itself.

## Three things this engine is careful about

**Elective deferrals reduce income tax wages, not FICA wages.** A pre-tax
401(k) deferral comes out of the base for federal and state income tax
withholding and out of nothing else. Social Security, Medicare and FUTA are all
computed on the full gross. This is the single most common error in a
hand-rolled payroll, and it produces a W-2 whose box 1 and box 3 are wrong in
opposite directions.

**No floating point touches a dollar.** Every monetary value is an integer
number of cents and every rate is an integer in parts-per-million. Rounding
happens once, at the published point, in the published direction.

**Rounding order is load-bearing.** The Yonkers resident surcharge is 16.75% of
the New York State tax, and composing it onto the *rounded* state figure gives
an answer one cent away from the one NYS-50-T-Y prints in its own worked
example. The state tax is therefore carried as an exact rational until the last
possible moment.

## Files

| path | what it is |
|------|-----------|
| `examples/rachuba.toml` | the invented starting configuration copied by `rachuba init` |
| `rachuba.toml` | the payer, the employee, the Form W-4 and the elections |
| `ledger.toml` | every committed run; the source of truth for wage bases and form lines |
| `tables/federal-<year>/` | Publication 15-T schedules, FICA, FUTA, retirement limits; `form-1040.toml` and `ira.toml` for the return projection |
| `tables/state-CA-<year>/` | DE 44 Method B tables, SDI, UI, ETT, FUTA credit reduction |
| `tables/state-NY-<year>/` | NYS-50-T-NYS, NYS-50-T-NYC, NYS-50-T-Y, DBL, PFL, SUI |

Each tax year is a folder of TOML section files, one per section of the
publication it transcribes (`sources.toml` carries the year, the citation and
the payroll periods). The loader merges every `.toml` file in the folder,
including sub-folders, into one document. It refuses, naming the files, a key
that two files both define, a file in the folder that is not `.toml`, and a
folder with no section file at all; it never picks one definition over another.
The folder is found through `RACHUBA_TABLES` when that is set, otherwise as
`tables` beside `rachuba.toml`. A set `RACHUBA_TABLES` that is not a directory
is refused rather than skipped, so a typo cannot make payroll read another
company's tables.

`rachuba init` writes both `rachuba.toml` and an empty `ledger.toml`, and refuses
to overwrite either. Every other command refuses to run without the ledger,
naming its path; a missing ledger is never read as "no runs yet".

Neither `rachuba.toml` nor `ledger.toml` is in this repository; both belong in
the company's own private one, and so do the household's figures in
`[household]`. Commit them there. A payroll you cannot reconstruct from a
commit is a payroll you cannot defend in an examination. The invented template
uses invalid EIN and SSN-last-four placeholders. Commands that load the
configuration refuse them until you enter the assigned EIN and actual last four
digits.

The source is licensed under Apache-2.0; see `LICENSE`. The private
configuration and ledger are not part of the distributed source.

This source repository ignores files named `rachuba.toml` and `ledger.toml` at
any depth, including a nested working directory. If you choose other names with
`--config` or `--ledger`, keep those files in your private repository or outside
this public checkout; the ignore rule cannot protect arbitrary paths.

No Social Security number is stored anywhere. Pay stubs carry the last four
digits; the full number is typed into SSA Business Services Online when the W-2
is filed.

## Source layout and verification

The public Rust module names and CLI commands stay unchanged. Internally,
`src/calculation/` owns exact arithmetic, federal/state computation and the
employee's own return (`personal/`), `src/records/` owns configuration and the
ledger, `src/reporting/` owns dates, forms and wage statements, and `src/cli/`
owns command dispatch and output.
Each state's table schema is separate from its withholding method; IRS legal
holidays remain distinct from the Department of Labor business-day definition.
Neither set is computed: both are the year's `legal-holidays.toml`, and a due
date that falls past the table's `covers_through` is refused, naming the
coverage and the folder where the next schedule belongs.

## Rolling to a new tax year

1. Copy `tables/federal-<year>/` to the new year and replace the numbers
   from that year's Publication 15-T percentage-method tables, the SSA wage base
   announcement, and the annual retirement limits; `form-1040.toml` from that
   year's inflation-adjustment revenue procedure, `ira.toml` from the IRS
   notice of the year's retirement limits, and `legal-holidays.toml` from
   OPM's federal holiday schedules for that year and the next (the year's
   obligations fall due into January of the next) with the District of
   Columbia's own legal holidays: Emancipation Day and, every fourth year,
   Inauguration Day.
2. Do the same for each state folder from that state's reissued publication.
   Keep every section file under 300 lines and at most five files per folder;
   add a sub-folder when a section outgrows that, as `state-CA-<year>/pit/` does.
3. Load the new tables with `rachuba`. The table loader rejects a schedule that
   is malformed, out of order, or missing its zero bracket.

No code changes. If a year's publication changes the *method* rather than the
numbers, that is a code change, and the method's shape is documented at the top
of the module that implements it.

## Sources

Every figure in the table files carries a citation in a comment naming the
publication, its revision date, and the page. Nothing is approximated, and a
figure that could not be verified is absent rather than guessed.

- IRS Publication 15-T (2026), Cat. No. 32112B, rev. 2025-12-03
- IRS Publication 15 (2026), section 11, deposit rules
- 26 CFR 31.6302-1, employment tax deposit schedules
- 26 CFR 31.6071(a)-1 and 26 U.S.C. 6071(c), return due dates
- 29 CFR 2510.3-102, participant contribution safe harbor
- 5 U.S.C. 6103, Executive Order 11582, OPM federal holiday schedules 2026
  and 2027, D.C. Code 1-612.02 and 26 U.S.C. 7503: the legal holidays
- Rev. Proc. 2025-32, sections 4.01, 4.03 and 4.14: 2026 rate schedules,
  capital gains thresholds, standard deduction
- IRS News Release IR-2025-111 and Notice 2025-67: 2026 IRA limit and
  phase-out ranges
- 26 U.S.C. 1(h), 1211(b), 1411, 3101(b)(2), 219(g), 408A(c)(3), 6654: capital
  gains rates, capital loss limit, net investment income tax, Additional
  Medicare Tax, IRA phase-out rounding, estimated tax
- DE 44, California Employer's Guide, 2026 edition, Rev. 52 (4-26)
- California Withholding Schedules for 2026, Method B, Exact Calculation Method
- Cal. Rev. & Tax. Code section 17041.5, statewide bar on local income tax
- US DOL ETA FUTA credit reduction determinations; Schedule A (Form 940) 2025
- NYS-50-T-NYS, NYS-50-T-NYC, NYS-50-T-Y, all rev. 1/26
- Form NYS-50 (12/25), New York unemployment wage base
- NYS Workers' Compensation Law section 209, disability contribution
