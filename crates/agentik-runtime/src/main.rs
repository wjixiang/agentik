//! `agentik-runtime` — the agent system portal binary.
//!
//! Two faces, one binary:
//! - `serve`   — run the long-lived daemon: agent process manager +
//!               control-plane REST + SSE. Other CLI invocations (and,
//!               later, the TUI) connect to it.
//! - `agent` / `daemon` — client subcommands that talk to a running daemon.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use agentik_api::{self as api, ContentBlock};
use agentik_control_client::ControlClient;
use agentik_runtime::{Runtime, RuntimeConfig, http, kinds};
use clap::{Parser, Subcommand};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

#[derive(Parser)]
#[command(
    name = "agentik-runtime",
    version,
    about = "Agent system portal: long-lived daemon + control-plane CLI"
)]
struct Cli {
    /// Tracing filter (RUST_LOG-style), e.g. `info`, `debug`, `agentik_runtime=trace`.
    #[arg(long, global = true, default_value = "info")]
    log: String,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run the runtime daemon: agent process manager + control-plane REST + SSE,
    /// until interrupted.
    Serve {
        /// Optional path to a `ModelConfig` JSON file used to configure the pool.
        #[arg(long)]
        config: Option<PathBuf>,
    },

    /// Manage a running daemon.
    Daemon {
        #[command(subcommand)]
        action: DaemonAction,
    },

    /// Control agents on a running daemon.
    Agent {
        #[command(subcommand)]
        action: AgentAction,
    },
}

#[derive(Subcommand)]
enum DaemonAction {
    /// Check whether the daemon is running (Ping).
    Status,
    /// Gracefully shut the daemon down.
    Stop,
}

#[derive(Subcommand)]
enum AgentAction {
    /// Spawn (and start) an agent by registered kind.
    Spawn {
        #[arg(long, default_value = "coder")]
        kind: String,
        /// Optional initial user message.
        #[arg(long)]
        message: Option<String>,
    },
    /// List managed agents.
    List,
    /// Inject a user message into an agent.
    Send {
        /// Agent id (UUID).
        id: String,
        /// Message text.
        text: String,
    },
    /// Stop a running agent.
    Stop { id: String },
    /// Get an agent's lifecycle status.
    Status { id: String },
    /// Subscribe to the event stream, filtered to one agent, until it exits.
    Follow { id: String },
    /// List models in the shared pool.
    Models,
    /// List registered agent kinds.
    Kinds,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_new(&cli.log).unwrap_or_else(|_| "info".into()),
        )
        .init();

    match cli.command {
        Command::Serve { config } => serve(config).await,
        Command::Daemon { action } => daemon(action).await,
        Command::Agent { action } => agent(action).await,
    }
}

// ── Daemon ─────────────────────────────────────────────────────

/// Run the runtime daemon until Ctrl-C or a `Shutdown` request.
async fn serve(config: Option<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    let mut rt_config = RuntimeConfig::headless();
    if let Some(path) = config {
        match load_model_config(&path) {
            Some(cfg) => rt_config = rt_config.with_model_config(cfg),
            None => tracing::warn!(path = %path.display(), "skipping model config"),
        }
    }

    let runtime = Runtime::new(rt_config).await?;

    // Register the built-in coder kind.
    runtime.registry().register(kinds::coder_kind());

    let pool_models = runtime.process_manager().pool_model_names().await;
    let pm = runtime.process_manager().clone();

    // ── Control-plane HTTP (REST + SSE) server ──
    let shutdown = CancellationToken::new();
    let control_listener = TcpListener::bind("0.0.0.0:0").await?;
    let control_addr = control_listener.local_addr()?;
    let state = http::HttpState::new(pm, shutdown.clone());
    let app = http::router(state);
    tokio::spawn(async move {
        if let Err(e) = axum::serve(control_listener, app).await {
            tracing::error!(error = %e, "control HTTP server error");
        }
    });

    // ── Write discovery file ──
    let info = api::DaemonInfo {
        control_addr,
        pid: std::process::id(),
        started_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
    };
    write_daemon_info(&info)?;

    println!("agentik-runtime daemon");
    println!("  control: http://{control_addr}");
    println!("  pool   : {} model(s)", pool_models.len());
    println!("  pid    : {}", info.pid);
    tracing::info!("daemon running — Ctrl-C or `daemon stop` to shut down");

    // ── Run until interrupted ──
    tokio::select! {
        _ = tokio::signal::ctrl_c() => println!("\nCtrl-C received, shutting down…"),
        _ = shutdown.cancelled() => println!("shutdown requested via control plane…"),
    }

    // ── Cleanup ──
    let _ = remove_daemon_info();
    let results = runtime.shutdown().await;
    println!("shutdown complete — {} agent(s) exited", results.len());
    Ok(())
}

/// Persist the discovery file.
fn write_daemon_info(info: &api::DaemonInfo) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(dir) = api::agentik_dir() {
        std::fs::create_dir_all(&dir)?;
    }
    let path = api::daemon_json_path()
        .ok_or_else(|| -> Box<dyn std::error::Error> { "no state dir".into() })?;
    std::fs::write(&path, serde_json::to_string_pretty(info)?)?;
    Ok(())
}

fn remove_daemon_info() -> std::io::Result<()> {
    if let Some(path) = api::daemon_json_path() {
        let _ = std::fs::remove_file(path);
    }
    Ok(())
}

// ── Daemon client subcommands ──────────────────────────────────

async fn daemon(action: DaemonAction) -> Result<(), Box<dyn std::error::Error>> {
    let info = api::read_daemon_info().ok_or("daemon not running (no discovery file)")?;
    let mut client = ControlClient::connect(&format!("http://{}", info.control_addr)).await?;
    match action {
        DaemonAction::Status => {
            client.ping().await?;
            println!(
                "daemon up — control http://{} pid {} (started {})",
                info.control_addr, info.pid, info.started_at
            );
        }
        DaemonAction::Stop => {
            client.shutdown().await?;
            println!("shutdown requested");
        }
    }
    Ok(())
}

// ── Agent client subcommands ───────────────────────────────────

async fn agent(action: AgentAction) -> Result<(), Box<dyn std::error::Error>> {
    let mut client = agentik_control_client::connect_to_daemon()
        .await
        .map_err(|e| format!("{e}"))?;
    match action {
        AgentAction::Spawn { kind, message } => {
            use agentik_api::AgentSpawnOpts;
            let opts = AgentSpawnOpts {
                initial_message: message.map(|t| vec![ContentBlock::Text { text: t }]),
                ..Default::default()
            };
            let id = client
                .spawn_agent(&kind, &opts)
                .await
                .map_err(|e| format!("{e}"))?;
            client.start_agent(id).await.map_err(|e| format!("{e}"))?;
            println!("spawned + started {kind} agent: {id}");
        }
        AgentAction::List => {
            let agents = client.list_agents().await.map_err(|e| format!("{e}"))?;
            if agents.is_empty() {
                println!("(no agents)");
            }
            for a in agents {
                println!("{}  {:?}  {}", a.agent_id, a.status, a.kind);
            }
        }
        AgentAction::Send { id, text } => {
            let id = uuid::Uuid::parse_str(&id)?;
            client
                .inject_message(id, vec![ContentBlock::Text { text }])
                .await
                .map_err(|e| format!("{e}"))?;
            println!("sent");
        }
        AgentAction::Stop { id } => {
            let id = uuid::Uuid::parse_str(&id)?;
            client.stop_agent(id).await.map_err(|e| format!("{e}"))?;
            println!("stopped");
        }
        AgentAction::Status { id } => {
            let id = uuid::Uuid::parse_str(&id)?;
            let status = client.get_status(id).await.map_err(|e| format!("{e}"))?;
            println!("{status:?}");
        }
        AgentAction::Follow { id } => {
            let id = uuid::Uuid::parse_str(&id)?;
            use tokio_stream::StreamExt as _;
            let mut stream = client.stream_events().await.map_err(|e| format!("{e}"))?;
            while let Some(item) = stream.next().await {
                let event = item.map_err(|e| format!("{e}"))?;
                if !event_concerns(&event, id) {
                    continue;
                }
                print_event(&event);
                if let agentik_api::ProcessEvent::ProcessExited { agent_id, .. } = &event {
                    if *agent_id == id {
                        break;
                    }
                }
            }
        }
        AgentAction::Models => {
            let models = client.get_pool_models().await.map_err(|e| format!("{e}"))?;
            if models.is_empty() {
                println!("(pool not configured)");
            }
            for m in models {
                println!("- {m}");
            }
        }
        AgentAction::Kinds => {
            let kinds = client.list_kinds().await.map_err(|e| format!("{e}"))?;
            for k in kinds {
                println!("- {k}");
            }
        }
    }
    Ok(())
}

/// Does this `ProcessEvent` concern the given agent?
fn event_concerns(event: &agentik_api::ProcessEvent, id: uuid::Uuid) -> bool {
    use agentik_api::ProcessEvent;
    match event {
        ProcessEvent::Agent { agent_id, .. }
        | ProcessEvent::StateChanged { agent_id, .. }
        | ProcessEvent::ProcessExited { agent_id, .. } => *agent_id == id,
    }
}

/// Print a one-line summary of a `ProcessEvent`.
fn print_event(event: &agentik_api::ProcessEvent) {
    use agentik_api::{AgentEvent, ProcessEvent};
    match event {
        ProcessEvent::StateChanged { new_status, .. } => {
            println!("[state] {new_status:?}");
        }
        ProcessEvent::ProcessExited { status, .. } => {
            println!("[exited] {status:?}");
        }
        ProcessEvent::Agent { event, .. } => match event {
            AgentEvent::TextDelta(t) => print!("{t}"),
            AgentEvent::ThinkingDelta(t) => print!("\x1b[2m{t}\x1b[0m"),
            AgentEvent::LlmResponse(s) => println!("[llm] {s}"),
            AgentEvent::Thinking(s) => println!("[think] {s}"),
            AgentEvent::ToolCall { name, input } => {
                println!(
                    "[tool] {name} {}",
                    serde_json::to_string(input).unwrap_or_default()
                );
            }
            AgentEvent::ToolResult { ok, content } => {
                println!("[result ok={ok}] {content}");
            }
            AgentEvent::Requesting => println!("[requesting]"),
            AgentEvent::Done => println!("[done]"),
            AgentEvent::Error(e) => println!("[error] {e}"),
            other => println!("[event] {other:?}"),
        },
    }
}

/// Read and parse a `ModelConfig` JSON file. Returns `None` on any error.
fn load_model_config(path: &Path) -> Option<agentik_runtime::ModelConfig> {
    let data = match std::fs::read_to_string(path) {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!(error = %e, path = %path.display(), "failed to read model config");
            return None;
        }
    };
    match serde_json::from_str::<agentik_runtime::ModelConfig>(&data) {
        Ok(cfg) => Some(cfg),
        Err(e) => {
            tracing::warn!(error = %e, path = %path.display(), "failed to parse model config");
            None
        }
    }
}
