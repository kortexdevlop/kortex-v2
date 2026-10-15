//! French private-sector gross <-> net salary estimate (2026 employee-side rates).
//!
//! Sources (verified 2026-10-15): PASS 2026 = 48 060 EUR / 4 005 EUR per month (arrete du
//! 22 decembre 2025); Agirc-Arrco 2026 rates (circulaire 2025-16): T1 3,15 %, T2 8,64 %,
//! CEG 0,86 % / 1,08 %, CET 0,14 %. CSG 6,80 % deductible + 2,40 % non deductible, CRDS 0,50 %
//! on 98,25 % of gross (capped at 4 PASS). Vieillesse de base 6,90 % (capped) / 0,40 % (uncapped)
//! and APEC 0,024 % are NOT re-verified against an official 2026 source: re-check each January.
//! Excluded: mutuelle, prevoyance, avantages en nature, Alsace-Moselle, public sector, forfaits.
use kortex::runtime::{Cx, ToolError};

use crate::{Inputs, Outputs, Params};

mod rates {
    pub const PASS_MONTHLY: f64 = 4005.0;
    pub const VIEILLESSE_PLAFONNEE: f64 = 0.069;
    pub const VIEILLESSE_DEPLAFONNEE: f64 = 0.004;
    pub const AA_T1: f64 = 0.0315;
    pub const AA_T2: f64 = 0.0864;
    pub const CEG_T1: f64 = 0.0086;
    pub const CEG_T2: f64 = 0.0108;
    pub const CET: f64 = 0.0014;
    pub const APEC: f64 = 0.00024;
    pub const CSG_DEDUCTIBLE: f64 = 0.068;
    pub const CSG_NON_DEDUCTIBLE: f64 = 0.024;
    pub const CRDS: f64 = 0.005;
    pub const CSG_BASE_FACTOR: f64 = 0.9825;
}

fn r2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

/// Monthly breakdown for a monthly gross.
#[derive(Debug, Clone, Copy)]
struct Breakdown {
    gross: f64,
    social: f64,
    csg_crds: f64,
    net_taxable: f64,
    net_before_tax: f64,
}

fn breakdown(gross: f64, cadre: bool) -> Breakdown {
    use rates::*;
    let t1 = gross.min(PASS_MONTHLY);
    let t2 = (gross - PASS_MONTHLY).clamp(0.0, 7.0 * PASS_MONTHLY);
    let mut social = r2(VIEILLESSE_PLAFONNEE * t1)
        + r2(VIEILLESSE_DEPLAFONNEE * gross)
        + r2(AA_T1 * t1)
        + r2(AA_T2 * t2)
        + r2(CEG_T1 * t1)
        + r2(CEG_T2 * t2);
    if gross > PASS_MONTHLY {
        social += r2(CET * (t1 + t2));
    }
    if cadre {
        social += r2(APEC * gross.min(4.0 * PASS_MONTHLY));
    }
    let cap = 4.0 * PASS_MONTHLY;
    let csg_base = CSG_BASE_FACTOR * gross.min(cap) + (gross - cap).max(0.0);
    let csg_deductible = r2(CSG_DEDUCTIBLE * csg_base);
    let csg_other = r2(CSG_NON_DEDUCTIBLE * csg_base) + r2(CRDS * csg_base);
    let net_taxable = gross - social - csg_deductible;
    Breakdown {
        gross,
        social,
        csg_crds: csg_deductible + csg_other,
        net_taxable,
        net_before_tax: net_taxable - csg_other,
    }
}

/// Smallest monthly gross giving `target` net (before tax).
///
/// Net is increasing in gross except for one small step down just above the PASS, where the
/// CET starts to apply: so search the part up to the PASS first, then the part above it.
fn gross_for_net(target: f64, cadre: bool) -> f64 {
    let net = |g: f64| breakdown(g, cadre).net_before_tax;
    let (mut lo, mut hi) = if target <= net(rates::PASS_MONTHLY) {
        (target, rates::PASS_MONTHLY)
    } else {
        (rates::PASS_MONTHLY, target * 3.0 + 1.0)
    };
    for _ in 0..80 {
        let mid = f64::midpoint(lo, hi);
        if net(mid) < target {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    f64::midpoint(lo, hi)
}

#[kortex::engine]
pub async fn run(_input: Inputs, params: Params, cx: &mut Cx) -> Result<Outputs, ToolError> {
    params.validate()?;
    cx.check_cancelled()?;
    let periods = match params.period.as_str() {
        "monthly" => 1.0,
        "annual" => 12.0,
        _ => return Err(ToolError::with_detail(ToolError::INVALID_PARAM, "period")),
    };
    let monthly_amount = params.amount / periods;
    let monthly_gross = match params.mode.as_str() {
        "gross_to_net" => monthly_amount,
        "net_to_gross" => gross_for_net(monthly_amount, params.cadre),
        _ => return Err(ToolError::with_detail(ToolError::INVALID_PARAM, "mode")),
    };
    let b = breakdown(monthly_gross, params.cadre);
    let income_tax = r2(b.net_taxable * params.tax_rate / 100.0);
    let out = |v: f64| r2(v * periods);
    Ok(Outputs {
        gross: out(b.gross),
        social_contributions: out(b.social),
        csg_crds: out(b.csg_crds),
        net_taxable: out(b.net_taxable),
        net_before_tax: out(b.net_before_tax),
        income_tax: out(income_tax),
        net_after_tax: out(b.net_before_tax - income_tax),
        charges_rate: if b.gross > 0.0 {
            r2((b.social + b.csg_crds) / b.gross * 100.0)
        } else {
            0.0
        },
    })
}
