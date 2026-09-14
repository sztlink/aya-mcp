use std::{env, path::PathBuf, sync::Arc, time::Duration};

use anyhow::{Context, Result, bail};
use aya_workcell::{RunConfig, Scenario, SystemClock, prepare_synthetic, run_synthetic};

fn argument(index: usize, default: &str) -> String {
    env::args().nth(index).unwrap_or_else(|| default.to_owned())
}

#[tokio::main]
async fn main() -> Result<()> {
    let root = PathBuf::from(argument(1, "target/aya-synthetic-workcell"));
    let scenario: Scenario = argument(2, "success").parse()?;
    if root.exists() {
        bail!(
            "refusing to reuse existing workcell directory {}",
            root.display()
        );
    }
    let current = env::current_exe()?;
    let fake_dcc = current
        .parent()
        .context("synthetic runner executable has no parent")?
        .join("aya-fake-dcc");
    let (score, lease) = prepare_synthetic(&root).await?;
    let report = run_synthetic(RunConfig {
        root,
        fake_dcc,
        score,
        lease,
        scenario,
        request_timeout: Duration::from_millis(250),
        clock: Arc::new(SystemClock),
    })
    .await?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
