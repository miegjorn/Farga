use farga_core::reader::{HttpFargaReader, FargaReader};
pub async fn run(base: &str, project: &str, since_hours: u64) -> anyhow::Result<()> {
    let reader = HttpFargaReader::new(base.to_string());
    let signals = reader.recent_signals(project, since_hours).await?;
    for s in &signals {
        println!("[{}] {}", s.source, s.content);
    }
    println!("{} signals", signals.len());
    Ok(())
}
