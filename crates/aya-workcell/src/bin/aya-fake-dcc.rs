use std::{env, path::PathBuf, process::Stdio, time::Duration};

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Command,
};

#[derive(Debug, Deserialize)]
struct Request {
    protocol: String,
    id: u64,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Debug, Serialize)]
struct Response {
    protocol: &'static str,
    id: u64,
    ok: bool,
    result: Value,
    error: Option<String>,
}

fn argument(name: &str) -> Result<String> {
    let args: Vec<String> = env::args().collect();
    let index = args
        .iter()
        .position(|value| value == name)
        .ok_or_else(|| anyhow!("missing {name}"))?;
    args.get(index + 1)
        .cloned()
        .ok_or_else(|| anyhow!("missing value for {name}"))
}

fn path_param(params: &Value, name: &str) -> Result<PathBuf> {
    params[name]
        .as_str()
        .map(PathBuf::from)
        .ok_or_else(|| anyhow!("missing path parameter {name}"))
}

async fn write_json(path: &PathBuf, value: &Value) -> Result<String> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let bytes = serde_json::to_vec_pretty(value)?;
    tokio::fs::write(path, &bytes).await?;
    Ok(aya_contracts::sha256_bytes(bytes))
}

async fn dispatch(
    request: &Request,
    scenario: &str,
    marker: &PathBuf,
    scene: &mut Value,
) -> Result<Value> {
    match request.method.as_str() {
        "discover" => Ok(json!({
            "runtime": "aya-fake-dcc/v0",
            "capabilities": ["inspect", "execute", "capture", "save"]
        })),
        "inspect" => {
            if scene.is_null() {
                let source = path_param(&request.params, "source")?;
                *scene = serde_json::from_slice(&tokio::fs::read(source).await?)?;
            }
            Ok(json!({"scene": scene}))
        }
        "execute" => {
            if scenario == "crash-once" && !marker.exists() {
                tokio::fs::write(marker, b"crashed\n").await?;
                std::process::exit(17);
            }
            if scenario == "timeout" {
                tokio::time::sleep(Duration::from_secs(60)).await;
            }
            if matches!(scenario, "source-mutation" | "mutation-then-crash") {
                let source = path_param(&request.params, "source")?;
                tokio::fs::write(source, b"{\"mutated\":true}\n").await?;
                if scenario == "mutation-then-crash" {
                    std::process::exit(19);
                }
            }
            let target = request.params["targetTension"]
                .as_i64()
                .ok_or_else(|| anyhow!("missing targetTension"))?;
            scene["tension"] = Value::Number(target.into());
            let mut result = json!({"scene": scene});
            if matches!(scenario, "orphan" | "detached-orphan") && target == 3 {
                let mut command = Command::new("sleep");
                command
                    .arg("300")
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null());
                if scenario == "detached-orphan" {
                    command.process_group(0);
                }
                let child = command
                    .spawn()
                    .context("could not spawn synthetic orphan")?;
                result["orphanPid"] = Value::Number(
                    child
                        .id()
                        .ok_or_else(|| anyhow!("synthetic orphan has no pid"))?
                        .into(),
                );
            }
            Ok(result)
        }
        "capture" => {
            let path = path_param(&request.params, "path")?;
            let label = request.params["label"].as_str().unwrap_or("capture");
            let evidence = json!({"label": label, "scene": scene});
            let mut digest = write_json(&path, &evidence).await?;
            if scenario == "inconsistent-evidence" {
                digest = aya_contracts::ZERO_SHA256.to_owned();
            }
            Ok(json!({"sha256": digest}))
        }
        "save" => {
            let path = path_param(&request.params, "path")?;
            if scenario == "partial-output" {
                let partial = path.with_extension("partial");
                write_json(&partial, scene).await?;
                bail!("synthetic partial output");
            }
            if scenario == "path-escape" {
                return Ok(json!({"artifactPath": "../escaped.scene.json"}));
            }
            let saved_scene = if scenario == "candidate-mismatch" {
                json!({"camera": "moved", "scale": 99, "tension": -1})
            } else {
                scene.clone()
            };
            write_json(&path, &saved_scene).await?;
            let relative = request.params["relativePath"]
                .as_str()
                .ok_or_else(|| anyhow!("missing relativePath"))?;
            Ok(json!({"artifactPath": relative}))
        }
        "shutdown" => Ok(json!({"shutdown": true})),
        other => bail!("unknown fake DCC method {other}"),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let scenario = argument("--scenario")?;
    let marker = PathBuf::from(argument("--marker")?);
    let mut scene = Value::Null;
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let mut stdout = tokio::io::stdout();

    while let Some(line) = lines.next_line().await? {
        let request: Request = serde_json::from_str(&line)?;
        if request.protocol != "aya.fake-dcc/v0" {
            bail!("unsupported protocol {}", request.protocol);
        }
        let result = dispatch(&request, &scenario, &marker, &mut scene).await;
        let shutdown = request.method == "shutdown";
        let response = match result {
            Ok(result) => Response {
                protocol: "aya.fake-dcc/v0",
                id: request.id,
                ok: true,
                result,
                error: None,
            },
            Err(error) => Response {
                protocol: "aya.fake-dcc/v0",
                id: request.id,
                ok: false,
                result: Value::Null,
                error: Some(error.to_string()),
            },
        };
        stdout.write_all(&serde_json::to_vec(&response)?).await?;
        stdout.write_all(b"\n").await?;
        stdout.flush().await?;
        if shutdown {
            break;
        }
    }
    Ok(())
}
