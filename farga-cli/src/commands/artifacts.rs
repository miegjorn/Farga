pub async fn run(base: &str, project: &str) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let url = format!("{}/artifacts/{}", base, project);
    let items: Vec<serde_json::Value> = client.get(&url).send().await?.json().await?;
    for item in &items {
        println!("- {}", item["title"].as_str().unwrap_or("?"));
    }
    Ok(())
}
