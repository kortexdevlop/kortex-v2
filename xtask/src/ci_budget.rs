//! `cargo xtask ci-budget`: upper-bound GitHub Actions minutes per workflow (MASTER_PROMPT §11).
//! Reads `runs-on`, `timeout-minutes` and matrix size from the workflow text; no YAML crate.
//! Billing multipliers on private repositories: Linux x1, Windows x2, macOS x10.
use std::fs;

const FREE_MONTHLY_MINUTES: u32 = 2000;

#[derive(Debug, PartialEq, Eq)]
pub struct Job {
    pub name: String,
    pub minutes: u32,
    pub multiplier: u32,
    pub copies: u32,
}

impl Job {
    #[must_use]
    pub fn billed(&self) -> u32 {
        self.minutes * self.multiplier * self.copies
    }
}

fn multiplier(runs_on: &str) -> u32 {
    if runs_on.contains("macos") {
        10
    } else if runs_on.contains("windows") {
        2
    } else {
        1
    }
}

/// Worst-case jobs of one workflow. A matrix `runs-on` is priced with the most expensive OS
/// listed under `include`.
pub fn parse(text: &str) -> Vec<Job> {
    let mut jobs: Vec<Job> = Vec::new();
    let mut in_jobs = false;
    let mut os_list: Vec<String> = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        if indent == 0 {
            in_jobs = trimmed == "jobs:";
            continue;
        }
        if !in_jobs {
            continue;
        }
        if indent == 2 && trimmed.ends_with(':') {
            jobs.push(Job {
                name: trimmed.trim_end_matches(':').to_owned(),
                minutes: 360,
                multiplier: 1,
                copies: 1,
            });
            os_list.clear();
        } else if let Some(job) = jobs.last_mut() {
            if let Some(value) = trimmed
                .strip_prefix("timeout-minutes:")
                .filter(|_| indent == 4)
            {
                job.minutes = value.trim().parse().unwrap_or(360);
            } else if let Some(value) = trimmed.strip_prefix("- os:") {
                os_list.push(value.trim().to_owned());
                job.copies = u32::try_from(os_list.len()).unwrap_or(1);
                job.multiplier = os_list.iter().map(|o| multiplier(o)).max().unwrap_or(1);
            } else if trimmed.starts_with("matrix:") && trimmed.contains("fromJSON") {
                // Dynamic matrix: price the worst case (Linux + macOS + Windows).
                job.copies = 3;
                job.multiplier = 10;
            } else if let Some(value) = trimmed
                .strip_prefix("runs-on:")
                .filter(|v| indent == 4 && !v.contains("matrix"))
            {
                job.multiplier = multiplier(value);
            }
        }
    }
    jobs
}

pub fn run() -> Result<(), String> {
    let mut entries = fs::read_dir(".github/workflows")
        .map_err(|e| e.to_string())?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "yml"))
        .collect::<Vec<_>>();
    entries.sort();
    println!(
        "Borne haute par exécution (timeouts × multiplicateurs ; macOS ×10, Windows ×2). Budget gratuit : {FREE_MONTHLY_MINUTES} min/mois."
    );
    for path in entries {
        let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let jobs = parse(&text);
        let total: u32 = jobs.iter().map(Job::billed).sum();
        println!("\n{} — {total} min facturées au maximum", path.display());
        for job in jobs {
            println!(
                "  {:<14} {:>4} min × {:>2} × {} = {}",
                job.name,
                job.minutes,
                job.multiplier,
                job.copies,
                job.billed()
            );
        }
    }
    println!(
        "\nCes valeurs sont des plafonds, pas des mesures : relever la durée réelle des runs avant d'ajuster."
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dynamic_matrix_is_priced_at_worst_case_and_blank_lines_are_ignored() {
        let yml = "jobs:\n  plan:\n    runs-on: ubuntu-latest\n    timeout-minutes: 5\n\n  build:\n    strategy:\n      matrix: ${{ fromJSON(x) }}\n    runs-on: ${{ matrix.os }}\n    timeout-minutes: 120\n";
        let jobs = parse(yml);
        assert_eq!(jobs.len(), 2);
        assert_eq!(jobs[1].billed(), 120 * 10 * 3);
    }

    #[test]
    fn prices_matrix_by_most_expensive_os_and_copies() {
        let yml = "name: x\njobs:\n  native:\n    strategy:\n      matrix:\n        include:\n          - os: ubuntu-latest\n          - os: macos-latest\n    runs-on: ${{ matrix.os }}\n    timeout-minutes: 10\n  web:\n    runs-on: ubuntu-latest\n    timeout-minutes: 5\n";
        let jobs = parse(yml);
        assert_eq!(jobs[0].billed(), 10 * 10 * 2);
        assert_eq!(jobs[1].billed(), 5);
    }
}
