// Example program to demonstrate message serialization format

use beads_workers::types::*;

fn main() {
    println!("=== Worker Messages ===\n");

    let ready = WorkerMessage::ready("W1".to_string());
    println!("READY message:");
    println!("{}\n", serde_json::to_string_pretty(&ready).unwrap());

    let done = WorkerMessage::done("W1".to_string(), "beads-workers-abc".to_string(), 15234);
    println!("DONE message:");
    println!("{}\n", serde_json::to_string_pretty(&done).unwrap());

    let failed = WorkerMessage::failed(
        "W1".to_string(),
        "beads-workers-abc".to_string(),
        "Task execution failed: command not found".to_string(),
        1234,
    );
    println!("FAILED message:");
    println!("{}\n", serde_json::to_string_pretty(&failed).unwrap());

    println!("=== Orchestrator Messages ===\n");

    let task = OrchestratorMessage::task(
        "W1".to_string(),
        "beads-workers-abc".to_string(),
        2,
        "Implement feature X".to_string(),
    );
    println!("TASK message:");
    println!("{}\n", serde_json::to_string_pretty(&task).unwrap());

    let wait = OrchestratorMessage::wait("W1".to_string(), 30);
    println!("WAIT message:");
    println!("{}\n", serde_json::to_string_pretty(&wait).unwrap());

    let shutdown = OrchestratorMessage::shutdown("W1".to_string(), ShutdownReason::UserRequested);
    println!("SHUTDOWN message:");
    println!("{}\n", serde_json::to_string_pretty(&shutdown).unwrap());
}
