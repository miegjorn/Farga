// Optimizer agent — two tasks wired at startup in v0.2.0
// v0.1.0: stub
pub async fn run_write_triggered(_node_id: &str) {
    // TODO: scoped lint pass on subgraph around node_id
}

pub async fn run_scheduled_sweep() {
    // TODO: LLM-assisted full sweep, PR creation
}
