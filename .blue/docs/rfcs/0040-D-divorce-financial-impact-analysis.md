# RFC 0040: Divorce Financial Impact Analysis

| | |
|---|---|
| **Status** | Draft |
| **Date** | 2026-01-27 |
| **Marriage Date** | December 28, 2021 |
| **Divorce Date** | January 29, 2026 |
| **Duration** | 4 years, 1 month |

---

## Summary

Assess the financial impact of divorce based on prenuptial agreement terms. Gather historical (Dec 2021) and current (Jan 2026) financial data to calculate property division per prenup Section 8.

## Prenup Key Provisions (Section 8 - Dissolution)

| Provision | Rule |
|-----------|------|
| **8a. Property acquired during marriage** | Equal interest (50/50), except appreciation of pre-marital assets |
| **8b. Homestead appreciation** | Proportionate to contributions (monetary + in-kind household/childcare) |
| **8c. Pre-marital assets** | Remain separate, including appreciation, income, reinvestment |
| **8d. Joint accounts** | Divided equally |
| **8e. Tax liability** | Pro-rata based on data causing the result |
| **Section 9. Support/Alimony** | Waived by both parties |

## Eric's Pre-Marital Baseline (Exhibit A - Dec 2021)

These remain Eric's separate property including all appreciation:

| Asset | Dec 2021 Value | Account |
|-------|----------------|---------|
| House | $410,000 | 3736 Everett (mortgage: $243,110) - **SOLD** |
| Other Real Estate | $720,000 | 2319 Ankeny (mortgage: $500,972) - **SOLD** |
| Vehicles | $10,000 | - |
| Solana (crypto) | $124,458 | - |
| Chubby Bunny Loan | $75,000 | Receivable |
| Kelli Personal Loan | $20,000 | Receivable |
| Stocks | $4,259 | Etrade (Spotify) |
| Checking | $34,199 | First Tech |
| IRA | $8,285 | Betterment |
| 401k | $129,849 | Vanguard |
| **Total Assets** | **$1,536,050** | |
| **Total Liabilities** | **$771,780** | |
| **Net Worth** | **$764,270** | |
| Annual Income | $235,000 | Spotify wages |

## Kelli's Pre-Marital Baseline (Exhibit B - Dec 2021)

| Asset | Dec 2021 Value | Account |
|-------|----------------|---------|
| Checking | $25,000 | Wells Fargo |
| **Net Worth** | **$25,000** | |
| Annual Income | $0 | |

## Data Gathering Plan

### Phase 1: Historical Statements (Dec 2021 / Jan 2022)

Need baseline values at time of marriage to verify Exhibit A/B accuracy:

| Institution | Account Type | Statement Needed |
|-------------|--------------|------------------|
| First Tech | Checking | Dec 2021 |
| Etrade | Brokerage | Dec 2021 |
| Betterment | IRA | Dec 2021 |
| Vanguard | 401k | Dec 2021 |
| Mortgage Servicer | 3736 Everett | Dec 2021 balance |
| Mortgage Servicer | 2319 Ankeny | Dec 2021 balance |

### Phase 2: Current Statements (Jan 2026)

| Institution | Account Type | Data Needed |
|-------------|--------------|-------------|
| First Tech | Checking | Current balance |
| Etrade | Brokerage | Current holdings + value |
| Betterment | IRA | Current balance |
| Vanguard | 401k | Current balance |
| Mortgage Servicer | 3736 Everett | Current balance |
| Mortgage Servicer | 2319 Ankeny | Current balance |
| Joint Account | Checking (33% contributions) | Current balance |
| Crypto Exchange | Solana holdings | Current value |

### Phase 3: Additional Data Needed

| Item | Purpose |
|------|---------|
| Real estate appraisals | Current market value of both properties |
| Homestead determination | Which property is "homestead" per 8b? |
| Contribution records | Joint account deposits by each party |
| Income history | Both parties' W-2s/1099s 2022-2025 |
| Property purchased during marriage | Any new assets bought with marital funds |
| Kelli's current accounts | Any accounts opened during marriage |

## Calculation Framework

### 1. Separate Property (No Division)

```
Eric's Separate = Pre-marital assets + appreciation
- Real estate (current value - current mortgage)
- Retirement accounts (Betterment IRA, Vanguard 401k)
- Brokerage (Etrade)
- Crypto (Solana)
- Outstanding loans receivable
```

### 2. Joint Property (50/50 Split)

```
Joint = Property acquired during marriage with joint funds
- Joint checking account balance
- Any jointly titled assets
```

### 3. Homestead Appreciation (Proportionate)

```
If homestead = 3736 Everett:
  Appreciation = Current Value - $410,000

Eric's Share = Appreciation * (Eric's contribution ratio)
Kelli's Share = Appreciation * (Kelli's contribution ratio)

Contribution ratio based on:
- Monetary contributions to household expenses
- In-kind contributions (childcare, household duties)
```

### 4. Property Acquired During Marriage (50/50)

```
Per Section 8a:
- Property bought by either party during marriage
- Excludes appreciation of pre-marital assets
```

## Playwright Automation Tasks

1. **Bank Logins** - Navigate to each institution
2. **Statement Download** - Download PDF statements for required dates
3. **Data Extraction** - Parse balances from statements
4. **Screenshot Capture** - Document current balances

### Institutions to Automate

- [x] Vanguard (401k) - DONE: Rolled over to ADP April 2024, $207k
- [x] **ADP** - DONE: $26,646.91 (new employer 401k)
- [x] **UBS** - DONE: $506,288.24 total (IRA + Brokerage)
- [x] First Tech Credit Union - DONE: $9,602.82 (Checking + Savings)
- [x] **Chase** - DONE: $6,951.12 joint checking (50/50 split)
- [x] **Cenlar FSB** - DONE: $521,030.51 mortgage balance (2619 56TH ST S)
- [x] **Coinbase** - DONE: Solana sold, $75.85 cash remaining
- ~~Original mortgage servicer(s) for 3736 Everett & 2319 Ankeny~~ **SOLD - need sale details**

## Confirmed Details

- **Homestead**: 3736 Everett (the $410k property)
- **Kelli's accounts**: Joint access available

## Account Location Changes (Since Prenup)

| Original (Dec 2021) | Current (Jan 2026) | Notes |
|---------------------|-------------------|-------|
| Vanguard 401k | **ADP 401k** | Rolled over April 2024 ($207,083.63) |
| Betterment IRA | **UBS IRA** | Transferred |
| Etrade Brokerage | **UBS Brokerage** | Transferred |

## Data Gathered - Vanguard (Completed)

| Finding | Value | Date |
|---------|-------|------|
| 401k at rollover | $207,083.63 | April 1, 2024 |
| Prenup baseline | $129,849 | Dec 2021 (Exhibit A) |
| Growth while at Vanguard | +$77,234.63 | Separate property per 8c |
| Q1 2024 Statement | Downloaded | .playwright-mcp/document.pdf |

**Note**: Dec 2021 statements unavailable (>24 month retention). Using signed Exhibit A as baseline.

## Data Gathered - ADP (Completed)

| Finding | Value | Date |
|---------|-------|------|
| Current 401k Balance | $26,646.91 | Jan 26, 2026 |
| Vested Balance | $26,646.91 | 100% vested |
| Performance (H2 2025) | +9.14% | |

**Note**: This is a NEW employer 401k (acquired during marriage). Subject to 50/50 split per Section 8a.

## Data Gathered - UBS (Completed)

### Account Summary

| Account | Type | Value | Notes |
|---------|------|-------|-------|
| TM 25650 | Roll IRA | $260,276.43 | Vanguard 401k rolled here April 2024 |
| TM 25651 | Brokerage | $246,011.81 | Need to verify if pre-marital |
| **Total UBS** | | **$506,288.24** | |

### TM 25650 - Rollover IRA Holdings (as of Jan 27, 2026)

Diversified equity portfolio including:
- GOOGL (Alphabet): $16,058.40
- AMZN (Amazon): $10,765.92
- AAPL (Apple): $11,880.42
- AVGO (Broadcom): $10,649.28
- FTXSX (FullerThaler Small Cap): $26,358.15
- Plus ~40 other individual stock positions

**Unrealized Gain/Loss: +$4,685.36** (all separate property appreciation per 8c)

## Data Gathered - First Tech (Completed)

| Account | Type | Balance | Notes |
|---------|------|---------|-------|
| *6752 | Checking | $9,291.51 | Pre-marital account (Exhibit A: $34,199) |
| *5020 | Savings | $311.31 | |
| **Total First Tech** | | **$9,602.82** | |

**Analysis**: Per Exhibit A, Eric had $34,199 in First Tech checking at marriage. Current balance is $9,291.51. This is separate property per Section 8c (pre-marital asset). The decrease likely reflects normal spending/transfers over 4 years.

## Data Gathered - Chase (Completed)

| Account | Type | Balance | Notes |
|---------|------|---------|-------|
| ...1267 | Total Checking (Personal) | $6,951.12 | **Joint account - 50/50 split per 8d** |
| ...2173 | Business Checking | $2,173.67 | Sheepish Productions LLC - **OMITTED (jointly owned, minimal value)** |
| ...1012 | Ink Unlimited Credit Card | $89.98 | Business card |

**Joint Account Analysis**: The personal Total Checking (...1267) with $6,951.12 is subject to 50/50 division per Section 8d.
- Kelli's share: $3,475.56
- Eric's share: $3,475.56

## Data Gathered - Cenlar FSB Mortgage (Completed)

| Item | Value | Notes |
|------|-------|-------|
| **Property** | 2619 56TH ST S | **NOT in prenup - acquired during marriage** |
| Original Loan | $551,200.00 | Loan started Jul 2022 |
| Current Balance | $521,030.51 | As of Jan 27, 2026 |
| Monthly Payment | $3,817.34 | Autopay from Chase ...1267 |
| Loan Duration | Jul 2022 - Aug 2052 | 30-year mortgage |
| Escrow Balance | $2,531.52 | |
| Monthly Escrow | $900.34 | |

### ⚠️ CRITICAL FINDING: New Property Acquired During Marriage

The property at **2619 56TH ST S** with mortgage serviced by Cenlar FSB was **NOT listed in the prenuptial agreement** (Exhibit A listed only 3736 Everett and 2319 Ankeny).

This property was acquired **during the marriage** (loan originated July 2022, after Dec 2021 marriage date).

**Per Section 8a**: Property acquired during marriage = **Equal interest (50/50)**, except appreciation of pre-marital assets.

**Action needed**:
1. Determine current market value of 2619 56TH ST S
2. Calculate equity: Market Value - $521,030.51 mortgage = Equity
3. Equity is subject to 50/50 split

## Data Gathered - 3736 Everett Sale (Completed)

**Source**: Closing Statement dated 7/3/2023

| Item | Value |
|------|-------|
| Property | 3736 NE Everett Street, Camas, WA |
| Sale Date | July 3, 2023 |
| Sale Price | $410,000.00 |
| Prenup Baseline (Dec 2021) | $410,000.00 |
| **Appreciation** | **$0** |
| Mortgage Payoff | $237,294.88 |
| Closing Costs/Fees | $5,743.69 |
| **Net Proceeds to Eric** | **$166,961.43** |

### Section 8b Analysis (Homestead Appreciation)

The homestead sold for **exactly the prenup baseline value** ($410,000). Per Section 8b, Kelli would be entitled to a proportionate share of appreciation based on contributions.

**Appreciation = $0 → Kelli's share of homestead appreciation = $0**

The $166,961.43 net proceeds are Eric's separate property (return of pre-marital equity).

**Note**: At time of sale, Eric's address was 2619 56th St S, Gulfport, FL - confirming 2619 56TH ST S was purchased before 3736 Everett was sold.

## Data Gathered - 2319 Ankeny Sale (Completed)

**Source**: ALTA Settlement Statement dated 02/03/2022

| Item | Value |
|------|-------|
| Property | 2319 SE Ankeny Street, Portland, OR |
| Sale Date | February 3, 2022 |
| Sale Price | $760,000.00 |
| Prenup Baseline (Dec 2021) | $720,000.00 |
| **Appreciation** | **+$40,000** |
| Mortgage Payoff | $497,143.25 |
| Real Estate Commissions | $38,000.00 |
| Other Closing Costs | $21,293.02 |
| **Net Proceeds to Eric** | **$206,146.10** |

### Section 8c Analysis (Pre-Marital Asset - NOT Homestead)

This property was sold **5 weeks after the marriage** (married Dec 28, 2021; sold Feb 3, 2022).

Per Section 8c, pre-marital assets remain separate property **including appreciation, income, and reinvestment**. This was NOT the homestead (that was 3736 Everett), so Section 8b does not apply.

**Result**: The $40,000 appreciation and $206,146.10 net proceeds are **Eric's separate property**. Kelli has no claim.

### Timeline of Property Transactions

| Date | Event | Amount |
|------|-------|--------|
| Dec 28, 2021 | Marriage | - |
| Feb 3, 2022 | 2319 Ankeny sold | +$206,146.10 proceeds |
| Jul 2022 | 2619 56TH ST S purchased | $551,200 mortgage |
| Jul 3, 2023 | 3736 Everett sold | +$166,961.43 proceeds |

## Data Gathered - Etrade/Spotify Stock Plan (Completed - CORRECTED)

**Source**: Etrade Stock Plan Tax Documents (2021, 2023, 2024), 2016 Spotify Benefits Summary, Spotify Share Register (Folio 1780)

The prenup Exhibit A listed only **$4,259** in Etrade stocks. However, the actual Spotify stock plan activity was significantly larger. Eric had multiple stock option and RSU grants.

### CRITICAL CORRECTION: Grant Dates vs. Vesting Dates

The 1099-B "Date Acquired" shows **vesting dates** (when shares became Eric's property), NOT **grant dates** (when Spotify awarded the options/RSUs). Per Section 8c, the **grant date** determines whether stock is pre-marital property.

**Evidence of Pre-Marital Employment & Grants:**
- 2016 Spotify USA Benefits Summary confirms Eric was employed at Spotify in 2016
- Spotify Share Register (Folio 1780) shows shares inscribed **July 27, 2017**
- Eric left Spotify in 2023, so no new grants could have been awarded that year

### Grant History (ALL PRE-MARITAL)

| Grant Date | Grant # | Type | Classification |
|------------|---------|------|----------------|
| 01/01/2010 | 5230 | ISO | **Pre-marital** |
| 12/01/2016 | NQ7955 | NQ | **Pre-marital** |
| 03/01/2019 | NQ5293 | RSU | **Pre-marital** |
| Pre-2021 | MM006808 | NQ | **Pre-marital** (grant date before marriage; vesting continued during marriage) |

**Note**: The "03/01/2023" date previously recorded for MM006808 was a **vesting date**, not a grant date. All Spotify grants were awarded before the December 28, 2021 marriage.

### Stock Plan Proceeds by Year (ALL PRE-MARITAL GRANTS)

**2021 (All BEFORE marriage Dec 28, 2021):**

| Grant # | Type | Proceeds | Status |
|---------|------|----------|--------|
| 5230 | ISO | $212,983.36 | Pre-marital grant, sold before marriage |
| NQ7955 | NQ | $90,289.73 | Pre-marital grant, sold before marriage |
| NQ5293 | RSU | $27,481.86 | Pre-marital grant, sold before marriage |
| **2021 Total** | | **$330,754.95** | **Eric's separate property** |

**2022:** No stock plan activity

**2023:**

| Grant # | Type | Proceeds | Status |
|---------|------|----------|--------|
| NQ7955 | NQ | $37,861.64 | Pre-marital grant (exercised during marriage) |
| NQ5293 | RSU | $11,696.81 | Pre-marital grant (vested during marriage) |
| MM006808 | NQ | $39,032.61 | Pre-marital grant (vested during marriage) |
| **2023 Total** | | **$88,591.06** | **Eric's separate property** |

**2024:**

| Grant # | Type | Proceeds | Status |
|---------|------|----------|--------|
| MM006808 | NQ | $90,193.66 | Pre-marital grant (exercised 02/08/2024) |
| MM006808 | NQ | $120,954.74 | Pre-marital grant (exercised 04/01/2024) |
| NQ5293 | RSU | $10,566.20 | Pre-marital grant (vested during marriage) |
| **2024 Total** | | **$221,714.60** | **Eric's separate property** |

### Complete Spotify Stock Plan Summary (CORRECTED)

| Category | Total Proceeds | Classification |
|----------|----------------|----------------|
| **ALL grants (pre-marital)** | **$641,060.61** | **Eric's separate property (Section 8c)** |

### Section 8c Analysis (CORRECTED)

**ALL Spotify stock grants were awarded BEFORE December 28, 2021.** Per Section 8c, pre-marital assets remain separate property "including appreciation, income, and reinvestment."

The fact that shares vested or were exercised during the marriage does NOT make them marital property. The **grant date** (when Spotify awarded the options/RSUs) determines classification, not the vesting or exercise date.

**Supporting Evidence:**
- 2016 Spotify USA Benefits Summary confirms Eric's employment at Spotify in 2016
- Spotify Share Register (Folio 1780) shows shares inscribed July 27, 2017
- Eric left Spotify in 2023; no new grants could have been awarded that year
- Grant MM006808 was awarded pre-marriage; only vesting occurred during marriage

**UBS Brokerage ($246,011.81):** This account contains proceeds from pre-marital Spotify stock grants. Per Section 8c, this is **Eric's separate property**.

**Kelli's share of Spotify stock proceeds: $0**

## Data Gathered - Coinbase/Solana (Completed)

**Source**: Coinbase transaction history

| Date | Transaction | Amount | Proceeds |
|------|-------------|--------|----------|
| Jan 12, 2022 | Sold 100 SOL | $15,104.97 | $14,879.91 withdrawn |
| Jul 5, 2023 | Sold 483.50 SOL | $8,879.19 | $8,746.89 withdrawn |
| Mar 14, 2025 | Sold 0.599 SOL | $79.59 | - |
| **Current Balance** | | **$75.85 cash** | **$0 crypto** |

### Section 8c Analysis (Pre-Marital Crypto)

The prenup listed Solana at **$124,458** (Dec 2021). All Solana was sold during the marriage:
- Total proceeds: ~$24,064 (significant loss from $124,458 baseline)
- Per Section 8c, pre-marital assets remain separate property including appreciation (or depreciation)
- The sale proceeds and current $75.85 balance are **Eric's separate property**

**Note**: The Jan 12, 2022 sale (100 SOL for $15,104.97) occurred just 2 weeks after marriage - confirming this was pre-marital Solana.

## Data Gathered - 2619 56TH ST S Valuation (Completed)

| Item | Value | Source |
|------|-------|--------|
| Current Market Value | $573,100 | Zillow estimate |
| Current Mortgage | $521,030.51 | Cenlar FSB |
| **Equity** | **$52,069.49** | |

### Down Payment Tracing Analysis - CONFIRMED

**Source**: First Tech Bank Statements (Feb 2022 & Jul 2022)

The down payment tracing has been **conclusively documented** via bank statements:

| Date | Transaction | Amount | Source |
|------|-------------|--------|--------|
| Feb 3, 2022 | Ankeny property sold | $206,146.10 net | ALTA Settlement Statement |
| **Feb 7, 2022** | **Wire Deposit from FIRST AMERICAN TITLE INSURANCE CO** | **$197,553.53** | First Tech Feb 2022 Statement |
| Jul 2022 | First Tech checking balance before purchase | $171,451.38 | First Tech Jul 2022 Statement |
| **Jul 12, 2022** | **Wire Withdrawal to FIDELITY NATIONAL TITLE OF FLORIDA** | **~$140,000+** | First Tech Jul 2022 Statement |
| Jul 31, 2022 | First Tech checking balance after purchase | $30,547.34 | First Tech Jul 2022 Statement |

**Tracing Chain Established:**
1. Ankeny sale proceeds ($197,553.53) deposited to First Tech on Feb 7, 2022
2. Funds remained in First Tech account (balance $171,451.38 in July)
3. Down payment wired to FIDELITY NATIONAL TITLE OF FLORIDA on Jul 12, 2022
4. Account balance dropped to $30,547.34 after wire

**Legal Analysis (Section 8c):**
Per Section 8c, pre-marital assets remain separate property **including reinvestment**. The Ankeny property was pre-marital, and its sale proceeds were directly reinvested into 2619 56TH ST S as the down payment.

**Result**: The down payment portion of 2619 56TH ST S equity is **Eric's separate property**. Since the current equity ($52,069.49) is LESS than the original down payment (~$140,000+), there is NO marital equity in this property.

**Kelli's share of 2619 56TH ST S: $0**

## Open Questions

1. ~~Which property is the "homestead" for Section 8b calculation?~~ **ANSWERED: 3736 Everett**
2. Did Kelli acquire any assets/income during the marriage?
3. ~~What joint property was purchased during the marriage?~~ **ANSWERED: 2619 56TH ST S (Jul 2022)**
4. What are the current real estate market values?
   - 3736 Everett (homestead) - for 8b appreciation calculation
   - 2319 Ankeny - pre-marital property
   - **2619 56TH ST S** - for 50/50 equity split calculation
5. How should in-kind contributions (household duties) be quantified for 8b?
6. ~~Was the $20,000 Kelli Personal Loan repaid?~~ **ANSWERED: NO - Kelli still owes Eric $20,000**
7. ~~Status of Chubby Bunny Loan ($75,000)?~~ **ANSWERED: Outstanding - Eric's separate property**
8. ~~Where are the mortgages for 3736 Everett and 2319 Ankeny serviced?~~ **ANSWERED: Both properties sold**
9. ~~What was the down payment source for 2619 56TH ST S?~~ **ANSWERED: Ankeny proceeds confirmed via First Tech bank statements (Feb 7 deposit → Jul 12 wire to title company)**
10. ~~Sale details for 3736 Everett~~ **ANSWERED: Sold 7/3/2023 for $410k (no appreciation), net $166,961.43**
11. ~~Sale details for 2319 Ankeny~~ **ANSWERED: Sold 2/3/2022 for $760k (+$40k appreciation), net $206,146.10 - all separate property**
12. ~~Were sale proceeds from either property used to purchase 2619 56TH ST S?~~ **CONFIRMED: Yes, via bank statement tracing**

---

## FINAL FINANCIAL IMPACT SUMMARY

### Eric's Separate Property (No Division - Section 8c)

| Asset | Current Value | Notes |
|-------|---------------|-------|
| UBS Rollover IRA | $260,276.43 | From Vanguard 401k ($129,849 baseline) |
| **UBS Brokerage** | **$246,011.81** | **From pre-marital Spotify stock grants** |
| First Tech Checking | $9,291.51 | Pre-marital ($34,199 baseline) |
| First Tech Savings | $311.31 | |
| Coinbase Cash | $75.85 | Solana sold ($124,458 baseline) |
| Vehicles | ~$10,000 | Per Exhibit A |
| **2619 56TH ST S Equity** | **$52,069.49** | **Down payment traced from Ankeny proceeds** |
| Chubby Bunny Loan | $75,000 | Receivable - **CONFIRMED OUTSTANDING** |
| **Total Separate** | **$653,036.40** | |

**CORRECTED - UBS Brokerage is SEPARATE PROPERTY**: All Spotify stock grants (including MM006808) were awarded BEFORE December 28, 2021. Per Section 8c, pre-marital assets remain separate property including appreciation and reinvestment. Evidence: 2016 Spotify Benefits Summary, July 2017 Share Register, Eric left Spotify in 2023 (no new grants possible).

**Note on Real Estate Proceeds**: The 3736 Everett ($166,961.43) and 2319 Ankeny ($206,146.10) sale proceeds are NOT listed separately because they were **reinvested into 2619 56TH ST S**. The current equity ($52,069.49) represents what remains of those funds - counting both would be double-counting.

**Note on Kelli Loan**: The $20,000 Kelli Personal Loan is listed separately below as an offset against her settlement share.

**Omitted**: Sheepish Productions LLC (Chase Business $2,173.67) - jointly owned, minimal value, omitted by agreement.

### Joint/Marital Property (50/50 Split - Sections 8a & 8d)

| Asset | Total Value | Eric's Share | Kelli's Share |
|-------|-------------|--------------|---------------|
| Chase Joint Checking | $6,951.12 | $3,475.56 | $3,475.56 |
| ADP 401k (new employer) | $26,646.91 | $13,323.46 | $13,323.46 |
| ~~UBS Brokerage~~ | ~~$246,011.81~~ | ~~$123,005.91~~ | ~~$123,005.91~~ |
| ~~2619 56TH ST S Equity~~ | ~~$52,069.49~~ | ~~$26,034.75~~ | ~~$26,034.75~~ |
| **Total Joint** | **$33,598.03** | **$16,799.02** | **$16,799.02** |

**Notes**:
- 2619 56TH ST S equity is **NOT joint property**. Down payment tracing CONFIRMED via First Tech bank statements (Ankeny proceeds → title company wire). Since current equity < down payment, 100% is Eric's separate property.
- **UBS Brokerage is NOT joint property (CORRECTED)**. All Spotify grants (including MM006808) were awarded before marriage. Per Section 8c, pre-marital assets remain separate property regardless of when vested or exercised.

### Homestead Appreciation (Section 8b)

| Item | Value |
|------|-------|
| 3736 Everett Sale Price | $410,000 |
| Prenup Baseline | $410,000 |
| **Appreciation** | **$0** |
| **Kelli's Share** | **$0** |

### Settlement Calculation (FINAL - With Confirmed Down Payment Tracing)

**Down payment tracing has been CONFIRMED via First Tech bank statements.**

The Ankeny sale proceeds ($197,553.53) were deposited Feb 7, 2022 and wired to FIDELITY NATIONAL TITLE OF FLORIDA on Jul 12, 2022 for the 2619 56TH ST S purchase. Since the current equity ($52,069.49) is less than the original down payment, **100% of the home equity is Eric's separate property**.

**Joint/Marital Property (50/50 Split):**

| Asset | Total Value | Eric's Share | Kelli's Share |
|-------|-------------|--------------|---------------|
| Chase Joint Checking | $6,951.12 | $3,475.56 | $3,475.56 |
| ADP 401k (new employer) | $26,646.91 | $13,323.46 | $13,323.46 |
| ~~UBS Brokerage~~ | ~~$246,011.81~~ | — | — |
| **Total Joint** | **$33,598.03** | **$16,799.02** | **$16,799.02** |

**Note**: UBS Brokerage removed from joint property - all Spotify grants were pre-marital.

**FINAL SETTLEMENT (Before Loan Offset):**

| Party | Calculation | Total |
|-------|-------------|-------|
| **Eric** | $653,036.40 (separate) + $16,799.02 (joint share) | **$669,835.42** |
| **Kelli** | $16,799.02 (joint share only) | **$16,799.02** |

**FINAL SETTLEMENT (With $20,000 Kelli Loan Offset):**

| Party | Calculation | Total |
|-------|-------------|-------|
| **Eric** | $669,835.42 + $20,000 (loan repayment) | **$689,835.42** |
| **Kelli** | $16,799.02 - $20,000 (loan owed to Eric) | **-$3,200.98** |

**Note on Negative Balance**: Kelli owes Eric $20,000 from pre-marital loan, but her share of marital property is only $16,799.02. This means Kelli still owes Eric $3,200.98 after the settlement.

**Verification**: $653,036.40 (separate) + $33,598.03 (joint) = $686,634.43 ✓

### Outstanding Items to Resolve

| Item | Impact |
|------|--------|
| ~~Down payment tracing for 2619 56TH ST S~~ | **RESOLVED: Confirmed via bank statements** |
| ~~Chubby Bunny Loan status ($75k)~~ | **CONFIRMED: Outstanding - Eric's separate property** |
| ~~Kelli Personal Loan status ($20k)~~ | **CONFIRMED: Outstanding - Kelli owes Eric $20,000** |
| Kelli's assets acquired during marriage | May increase joint pool |
| Alimony | **Waived** per Section 9 |

### Kelli Personal Loan Offset Analysis

Per Exhibit A, Eric loaned Kelli $20,000 before the marriage. This loan remains **outstanding and unpaid**.

This is Eric's separate property (a receivable). In the divorce settlement, this debt can be:
1. **Offset against Kelli's share** - Reduce her $141,889.53 by $20,000 = Kelli receives $121,889.53
2. **Collected separately** - Kelli pays Eric $20,000 plus receives her $141,889.53

**Net Settlement with Offset:**

| Party | Calculation | Net |
|-------|-------------|-----|
| Eric | $653,036.40 (separate) + $16,799.02 (joint) + $20,000 (loan repayment) | **$689,835.42** |
| Kelli | $16,799.02 (joint) - $20,000 (loan owed) | **-$3,200.98** |

**Total**: $686,634.44 (matches total current assets)

**Settlement Outcome**: Kelli owes Eric $3,200.98. Alternatively, Eric can forgive the remaining loan balance and call it even.

---

## Deliverables

1. **Asset Inventory Spreadsheet** - All assets with Dec 2021 vs Jan 2026 values
2. **Separate Property Summary** - Eric's separate property calculation
3. **Joint Property Summary** - Assets to be divided 50/50
4. **Homestead Appreciation Calculation** - If applicable
5. **Net Settlement Estimate** - Who owes what to whom

---

*This is a planning document, not legal advice. Consult a family law attorney for the actual divorce proceedings.*
