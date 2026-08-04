use crate::doctor;
use crate::output::{emit_success, CliContext, CliExitCode, OutputMode};
use std::process::ExitCode;

pub async fn run_doctor(ctx: &CliContext, offline: bool) -> ExitCode {
    let report = doctor::run_checks(std::path::Path::new(""), offline).await;

    match ctx.mode {
        OutputMode::Human => print_human_doctor_report(&report),
        OutputMode::Json => emit_success(ctx, report.clone()),
    }

    if report.has_failures() {
        CliExitCode::RuntimeError.into()
    } else {
        CliExitCode::Success.into()
    }
}

fn print_human_doctor_report(report: &doctor::DoctorReport) {
    println!("ResMate doctor");
    println!();
    for check in &report.checks {
        let icon = match check.status.as_str() {
            "pass" => "OK",
            "warn" => "WARN",
            "fail" => "FAIL",
            "skip" => "SKIP",
            _ => "?",
        };
        println!("[{}] {} — {}", icon, check.id, check.message);
    }
}
