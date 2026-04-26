use crate::{
    client::{
        app_client, fetch_json, post_empty_json, resolve_base_url, AppRequest, ResolvedBaseUrl,
    },
    error::{CliError, CliFailure},
};
use clap::{Args, Subcommand};
use reqwest::Client;
use serde_json::{json, Value};

#[derive(Args, Debug)]
#[command(
    about = "Manage runtime jobs through the App API",
    after_help = "Examples:\n  faus jobs list\n  faus jobs list --library-id demo\n  faus jobs show job_000001"
)]
pub(crate) struct JobsArgs {
    #[command(subcommand)]
    command: JobsCommand,
}

#[derive(Subcommand, Debug)]
enum JobsCommand {
    #[command(about = "List jobs")]
    List(ListJobsArgs),
    #[command(about = "Show one job")]
    Show(JobIdArgs),
    #[command(about = "Cancel a job")]
    Cancel(JobIdArgs),
    #[command(about = "Resume a terminal retryable job")]
    Resume(JobIdArgs),
    #[command(about = "Retry a terminal retryable job")]
    Retry(JobIdArgs),
}

#[derive(Args, Debug)]
struct ListJobsArgs {
    #[arg(long, value_name = "LIBRARY_ID", help = "Filter jobs by library id")]
    library_id: Option<String>,
}

#[derive(Args, Debug)]
struct JobIdArgs {
    #[arg(value_name = "JOB_ID", help = "Job id")]
    job_id: String,
}

pub(crate) async fn run_jobs(
    args: JobsArgs,
    base_url_arg: Option<String>,
    json_output: bool,
    debug: bool,
) -> Result<(), CliFailure> {
    let base =
        resolve_base_url(base_url_arg).map_err(|error| CliFailure::client(error, json_output))?;
    let client = app_client().map_err(|error| CliFailure::client(error, json_output))?;
    let response = execute_jobs_command(&client, &base, args.command)
        .await
        .map_err(|error| CliFailure::client(error, json_output))?;

    if json_output {
        print_json_output(&base.base_url, base.source, &response, debug);
    } else {
        print_human_output(response);
    }

    Ok(())
}

async fn execute_jobs_command(
    client: &Client,
    base: &ResolvedBaseUrl,
    command: JobsCommand,
) -> Result<JobsResponse, CliError> {
    match command {
        JobsCommand::List(args) => {
            let path = match args.library_id {
                Some(library_id) => format!("/jobs?library_id={library_id}"),
                None => "/jobs".to_string(),
            };
            let request = base.request(path);
            let fetched = fetch_json(client, &request).await?;
            let jobs = jobs_from_envelope(&fetched.value, &request)?;
            Ok(JobsResponse {
                request_url: request.url,
                http_status: fetched.status,
                output: JobsOutput::List { jobs },
            })
        }
        JobsCommand::Show(args) => {
            let request = base.request(format!("/jobs/{}", args.job_id));
            let fetched = fetch_json(client, &request).await?;
            let job = job_from_envelope(&fetched.value, &request)?;
            Ok(JobsResponse {
                request_url: request.url,
                http_status: fetched.status,
                output: JobsOutput::Job { job },
            })
        }
        JobsCommand::Cancel(args) => action_job(client, base, &args.job_id, "cancel").await,
        JobsCommand::Resume(args) => action_job(client, base, &args.job_id, "resume").await,
        JobsCommand::Retry(args) => action_job(client, base, &args.job_id, "retry").await,
    }
}

async fn action_job(
    client: &Client,
    base: &ResolvedBaseUrl,
    job_id: &str,
    action: &str,
) -> Result<JobsResponse, CliError> {
    let request = base.request(format!("/jobs/{job_id}/{action}"));
    let fetched = post_empty_json(client, &request).await?;
    let job = job_from_envelope(&fetched.value, &request)?;
    Ok(JobsResponse {
        request_url: request.url,
        http_status: fetched.status,
        output: JobsOutput::Job { job },
    })
}

fn jobs_from_envelope(value: &Value, request: &AppRequest) -> Result<Value, CliError> {
    let Some(jobs) = value.get("data").and_then(|data| data.get("jobs")) else {
        return Err(invalid_success_envelope(request, "data.jobs array"));
    };
    if !jobs.is_array() {
        return Err(invalid_success_envelope(request, "data.jobs array"));
    }
    Ok(jobs.clone())
}

fn job_from_envelope(value: &Value, request: &AppRequest) -> Result<Value, CliError> {
    let Some(job) = value.get("data") else {
        return Err(invalid_success_envelope(request, "data object"));
    };
    if !job.is_object() {
        return Err(invalid_success_envelope(request, "data object"));
    }
    Ok(job.clone())
}

fn invalid_success_envelope(request: &AppRequest, expected: &str) -> CliError {
    CliError::new(
        "invalid_response",
        format!(
            "{} did not return a SuccessEnvelope {expected}",
            request.url
        ),
    )
    .with_hint(
        "The server responded, but the payload did not match the FauniSearch App API contract.",
    )
    .with_details(request.details(None))
}

fn print_json_output(base_url: &str, base_url_source: &str, response: &JobsResponse, debug: bool) {
    let mut payload = match &response.output {
        JobsOutput::List { jobs } => json!({
            "status": "ok",
            "data": {
                "base_url": base_url,
                "jobs": jobs,
            },
        }),
        JobsOutput::Job { job } => json!({
            "status": "ok",
            "data": {
                "base_url": base_url,
                "job": job,
            },
        }),
    };
    if debug {
        payload["debug"] = json!({
            "base_url_source": base_url_source,
            "request_url": response.request_url,
            "http_status": response.http_status,
        });
    }
    println!(
        "{}",
        serde_json::to_string(&payload).expect("jobs JSON should serialize")
    );
}

fn print_human_output(response: JobsResponse) {
    match response.output {
        JobsOutput::List { jobs } => {
            let jobs = jobs.as_array().expect("jobs should be an array");
            if jobs.is_empty() {
                println!("No jobs.");
                return;
            }
            for job in jobs {
                println!("{}", job_summary(job));
            }
        }
        JobsOutput::Job { job } => println!("{}", job_summary(&job)),
    }
}

fn job_summary(job: &Value) -> String {
    let job_id = string_field(job, "job_id");
    let status = string_field(job, "status");
    let phase = string_field(job, "phase");
    let kind = string_field(job, "kind");
    let library_id = string_field(job, "library_id");
    let cancelable = bool_field(job, "cancelable");
    let retryable = bool_field(job, "retryable");
    let progress = progress_summary(job);
    let attempt = attempt_summary(job);
    format!(
        "{job_id}\t{status}\tphase={phase}\tkind={kind}\tlibrary={library_id}\tprogress={progress}\tcancelable={cancelable}\tretryable={retryable}\tattempt={attempt}"
    )
}

fn progress_summary(job: &Value) -> String {
    let progress = job.get("progress").unwrap_or(&Value::Null);
    let completed = progress
        .get("completed")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let total = progress.get("total").and_then(Value::as_u64).unwrap_or(0);
    let unit = progress
        .get("unit")
        .and_then(Value::as_str)
        .unwrap_or("unit");
    format!("{completed}/{total} {unit}")
}

fn attempt_summary(job: &Value) -> String {
    let attempt = job.get("current_attempt").unwrap_or(&Value::Null);
    let attempt_number = attempt.get("attempt").and_then(Value::as_u64).unwrap_or(0);
    let status = attempt
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let summary = attempt.get("summary").and_then(Value::as_str).unwrap_or("");
    if summary.is_empty() {
        format!("{attempt_number}:{status}")
    } else {
        format!("{attempt_number}:{status}:{summary}")
    }
}

fn string_field(value: &Value, field: &str) -> String {
    value
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string()
}

fn bool_field(value: &Value, field: &str) -> bool {
    value.get(field).and_then(Value::as_bool).unwrap_or(false)
}

struct JobsResponse {
    request_url: String,
    http_status: u16,
    output: JobsOutput,
}

enum JobsOutput {
    List { jobs: Value },
    Job { job: Value },
}
