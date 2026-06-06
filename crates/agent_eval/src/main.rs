//! Eval harness CLI.
//!
//! Usage:
//!   cargo run -p agent_eval -- --label baseline
//!   cargo run -p agent_eval -- --label after
//!
//! Writes `crates/agent_eval/reports/<label>.json` and, when both baseline and after exist,
//! prints a delta table proving the Phase 5 targets.

use std::path::PathBuf;

use agent_eval::{EvalReport, run_eval};

fn main() {
    let mut label = "baseline".to_string();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--label" => {
                if let Some(v) = args.next() {
                    label = v;
                }
            }
            other => eprintln!("ignoring unknown arg: {other}"),
        }
    }

    let report = run_eval(&label);
    let reports_dir = reports_dir();
    std::fs::create_dir_all(&reports_dir).expect("create reports dir");
    let out_path = reports_dir.join(format!("{label}.json"));
    let json = serde_json::to_string_pretty(&report).expect("serialize report");
    std::fs::write(&out_path, &json).expect("write report");

    print_report(&report);
    println!("\nWrote report to {}", out_path.display());

    // If we now have both baseline and after, print the delta.
    let baseline_path = reports_dir.join("baseline.json");
    let after_path = reports_dir.join("after.json");
    if baseline_path.exists() && after_path.exists() {
        if let (Ok(b), Ok(a)) = (read_report(&baseline_path), read_report(&after_path)) {
            print_delta(&b, &a);
        }
    }
}

fn reports_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("reports")
}

fn read_report(path: &PathBuf) -> Result<EvalReport, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

fn print_report(r: &EvalReport) {
    println!("=== eval report: {} ===", r.label);
    println!(
        "{:<18} {:>12} {:>14} {:>10} {:>10} {:>8} {:>9}",
        "scenario", "tokens/task", "tool_tok/turn", "traj_len", "tool_acc", "arg_f1", "recovery"
    );
    for s in &r.scenarios {
        println!(
            "{:<18} {:>12} {:>14} {:>10} {:>10.2} {:>8.2} {:>9.2}",
            s.name,
            s.tokens_per_task,
            s.avg_tool_tokens_per_turn,
            s.trajectory_length,
            s.tool_selection_accuracy,
            s.argument_f1,
            s.recovery_rate,
        );
    }
    println!(
        "{:<18} {:>12} {:>14} {:>10.2} {:>10.2} {:>8.2} {:>9.2}",
        "TOTAL/MEAN",
        r.total_tokens_per_task,
        "-",
        r.mean_trajectory_length,
        r.mean_tool_selection_accuracy,
        r.mean_argument_f1,
        r.mean_recovery_rate,
    );
}

fn print_delta(b: &EvalReport, a: &EvalReport) {
    println!("\n=== baseline → after ===");
    let pct = |before: f64, after: f64| -> f64 {
        if before == 0.0 {
            0.0
        } else {
            (after - before) / before * 100.0
        }
    };
    let tok = pct(
        b.total_tokens_per_task as f64,
        a.total_tokens_per_task as f64,
    );
    println!(
        "tokens/task:        {} → {}  ({:+.1}%)",
        b.total_tokens_per_task, a.total_tokens_per_task, tok
    );
    println!(
        "trajectory (mean):  {:.2} → {:.2}  ({:+.1}%)",
        b.mean_trajectory_length,
        a.mean_trajectory_length,
        pct(b.mean_trajectory_length, a.mean_trajectory_length)
    );
    println!(
        "tool accuracy:      {:.2} → {:.2}",
        b.mean_tool_selection_accuracy, a.mean_tool_selection_accuracy
    );
    println!(
        "argument F1:        {:.2} → {:.2}",
        b.mean_argument_f1, a.mean_argument_f1
    );
    println!(
        "recovery rate:      {:.2} → {:.2}",
        b.mean_recovery_rate, a.mean_recovery_rate
    );
    let target = -30.0;
    if tok <= target {
        println!("\n✅ tokens-per-task target met (≥30% reduction).");
    } else {
        println!(
            "\n⚠️  tokens-per-task reduction is {:.1}% (target ≥30%).",
            -tok
        );
    }
}
