use std::error::Error;
use std::path::PathBuf;

use clap::Parser;
use first_principles_agent::{TaskGraphService, home_or_legacy};

#[derive(Debug, Parser)]
#[command(
    name = "task-graph-inspect",
    about = "Read-only inspector for the 1Agent task-graph event log"
)]
struct Cli {
    #[arg(long, default_value_os_t = home_or_legacy("task-graph.jsonl", ".1a/task-graph.jsonl"))]
    store: PathBuf,
    #[arg(long)]
    json: bool,
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    let service = TaskGraphService::open_read_only(&cli.store)?;
    let snapshot = service.snapshot();
    if cli.json {
        println!("{}", serde_json::to_string_pretty(&snapshot)?);
        return Ok(());
    }
    println!("store: {}", service.store_path().display());
    println!(
        "threads={} evals={} tickets={} reports={} side_effects={}",
        snapshot.threads.len(),
        snapshot.evals.len(),
        snapshot.tickets.len(),
        snapshot.reports.len(),
        snapshot.side_effects.len()
    );
    for thread in &snapshot.threads {
        println!(
            "thread {} status={:?} triggers={} subscriptions={} tickets={}",
            thread.thread_id,
            thread.status,
            thread.trigger_ids.len(),
            thread.subscription_ids.len(),
            thread.ticket_ids.len()
        );
    }
    for ticket in &snapshot.tickets {
        println!(
            "ticket {} status={:?} owner={} mandate={} assignment={}",
            ticket.ticket_id,
            ticket.status,
            ticket.owner_id,
            ticket.mandate_id,
            ticket.assignment_id.as_deref().unwrap_or("-")
        );
    }
    Ok(())
}
