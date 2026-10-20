use crate::{Inputs, Outputs, Params};
use kortex::runtime::{Cx, ToolError};

use kortex::runtime::Table;
#[kortex::engine]
pub async fn run(_input: Inputs, params: Params, cx: &mut Cx) -> Result<Outputs, ToolError> {
    params.validate()?;
    cx.check_cancelled()?;
    if ![params.principal, params.annual_rate, params.insurance_rate]
        .iter()
        .all(|n| n.is_finite())
    {
        return Err(ToolError::new("invalid_param"));
    }
    let months = params.years * 12;
    let n = months as f64;
    let rate = params.annual_rate / 1200.0;
    let payment = if rate == 0.0 {
        params.principal / n
    } else {
        params.principal * rate / (-(-n * rate.ln_1p()).exp_m1())
    };
    let insurance_month = params.principal * params.insurance_rate / 1200.0;
    let mut balance = params.principal;
    let mut interest = 0.0;
    let mut rows = Vec::new();
    for month in 1..=months {
        cx.check_cancelled()?;
        let fee = balance * rate;
        let paid = if month == months {
            balance
        } else {
            (payment - fee).min(balance).max(0.0)
        };
        balance = (balance - paid).max(0.0);
        interest += fee;
        rows.push(vec![
            month.to_string(),
            format!("{paid:.2}"),
            format!("{fee:.2}"),
            format!("{balance:.2}"),
        ]);
    }
    let insurance = insurance_month * n;
    Ok(Outputs {
        monthly: payment + insurance_month,
        interest,
        insurance,
        total: params.principal + interest + insurance,
        schedule: Table {
            columns: vec![
                "month".into(),
                "principal".into(),
                "interest".into(),
                "balance".into(),
            ],
            rows,
        },
    })
}
