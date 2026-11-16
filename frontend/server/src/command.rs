//! Running the tests of a batch from the frontend, and what it finds out about the sources
//! on the way: where the workspace is, and what git says of it.
//!
//! "Run again" in the UI executes the command of the batch, `cargo test --no-fail-fast`
//! unless the batch says otherwise, in the directory of the batch. The tests are told to
//! report here, to that batch. What the command prints is passed on to the UIs.

use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;

use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::Command;
use tokio::sync::Notify;

use crate::store::{DEFAULT_BATCH, Sources};
use crate::{CommandRun, Frontend, OUTPUT_KEPT, or_complain};

/// The workspace a package belongs to: the nearest directory, from the package up, that has
/// a `Cargo.lock`. `None` if the package is not on this machine.
pub fn workspace_dir(manifest_dir: &Path) -> Option<String> {
    if !manifest_dir.is_dir() {
        return None;
    }
    let workspace = manifest_dir
        .ancestors()
        .find(|dir| dir.join("Cargo.lock").is_file())
        .unwrap_or(manifest_dir);
    Some(workspace.to_string_lossy().into_owned())
}

/// What git says of the sources in `dir`. Nothing, where git has nothing to say.
pub fn sources(dir: &str) -> Sources {
    let git = |args: &[&str]| {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .ok()
            .filter(|output| output.status.success())?;
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    };
    Sources {
        commit: git(&["rev-parse", "--short", "HEAD"]),
        branch: git(&["rev-parse", "--abbrev-ref", "HEAD"]).filter(|branch| branch != "HEAD"),
        dirty: git(&["status", "--porcelain"]).map(|changes| !changes.is_empty()),
    }
}

#[cfg(unix)]
fn shell(command: &str) -> Command {
    let mut shell = Command::new("sh");
    shell.arg("-c").arg(command);
    // A group of its own, so that all of it can be stopped: `cargo test`, and the test
    // binaries it started.
    shell.process_group(0);
    shell
}

#[cfg(not(unix))]
fn shell(command: &str) -> Command {
    let mut shell = Command::new("cmd");
    shell.arg("/C").arg(command);
    shell
}

/// Stops the command and everything it started.
async fn stop(child: &mut tokio::process::Child) {
    #[cfg(unix)]
    if let Some(pid) = child.id() {
        let group = format!("-{pid}");
        let _ = Command::new("kill")
            .args(["-TERM", "--", &group])
            .status()
            .await;
        return;
    }
    let _ = child.start_kill();
}

impl Frontend {
    fn command_printed(&self, batch: &str, line: String) {
        let mut hub = self.hub.lock().unwrap();
        hub.tell_ui(json!({ "kind": "command_output", "batch": batch, "lines": [line] }));
        let output = &mut hub.commands.entry(batch.to_string()).or_default().output;
        output.push_back(line);
        if output.len() > OUTPUT_KEPT {
            output.pop_front();
        }
    }

    fn command_is(&self, batch: &str, running: bool, code: Option<i32>) {
        let mut hub = self.hub.lock().unwrap();
        let command = hub.commands.entry(batch.to_string()).or_default();
        command.running = running;
        command.code = code;
        hub.tell_ui(
            json!({ "kind": "command_state", "batch": batch, "running": running, "code": code }),
        );
        hub.tell_batches();
    }

    async fn pass_on(&self, batch: &str, printed: impl AsyncRead + Unpin) {
        let mut lines = BufReader::new(printed).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            self.command_printed(batch, line);
        }
    }
}

/// Runs the command of the batch, unless it is running already or nothing may be run. Must
/// be called from within the runtime of the frontend.
pub(crate) fn run(frontend: &Frontend, batch: String) {
    let (address, command, dir, stop_asked) = {
        let mut hub = frontend.hub.lock().unwrap();
        let Some(address) = hub.run_address.clone() else {
            return;
        };
        if hub.commands.get(&batch).is_some_and(|run| run.running) {
            return;
        }
        let (command, dir) = or_complain(hub.store.command(&batch).map(Some))
            .unwrap_or_else(|| (crate::store::DEFAULT_COMMAND.to_string(), None));
        let stop = Arc::new(Notify::new());
        let run = CommandRun {
            running: true,
            stop: stop.clone(),
            ..CommandRun::default()
        };
        hub.commands.insert(batch.clone(), run);
        (address, command, dir, stop)
    };
    frontend.command_is(&batch, true, None);

    let frontend = frontend.clone();
    tokio::spawn(async move {
        let Some(dir) = dir else {
            let why = "no directory to run in: set one for this batch, or run its tests once \
                       from a terminal";
            frontend.command_printed(&batch, why.to_string());
            frontend.command_is(&batch, false, None);
            return;
        };
        frontend.command_printed(&batch, format!("{dir}$ {command}"));
        let mut shell = shell(&command);
        shell
            .current_dir(&dir)
            .env("TESTSCRIBE_FRONTEND", address)
            .env_remove("TESTSCRIBE_RUN")
            .env_remove("TESTSCRIBE_BATCH_ID")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        if batch != DEFAULT_BATCH {
            shell.env("TESTSCRIBE_BATCH_ID", &batch);
        }
        let mut child = match shell.spawn() {
            Ok(child) => child,
            Err(err) => {
                frontend.command_printed(&batch, format!("could not be started: {err}"));
                frontend.command_is(&batch, false, None);
                return;
            }
        };
        let stdout = child.stdout.take().expect("the output is piped");
        let stderr = child.stderr.take().expect("the errors are piped");
        let printed = async {
            tokio::join!(
                frontend.pass_on(&batch, stdout),
                frontend.pass_on(&batch, stderr)
            );
        };
        let ended = async {
            tokio::select! {
                status = child.wait() => status,
                () = stop_asked.notified() => {
                    stop(&mut child).await;
                    child.wait().await
                }
            }
        };
        let (status, ()) = tokio::join!(ended, printed);
        let code = status.ok().and_then(|status| status.code());
        frontend.command_printed(
            &batch,
            match code {
                Some(0) => "finished".to_string(),
                Some(code) => format!("finished with exit code {code}"),
                None => "stopped".to_string(),
            },
        );
        frontend.command_is(&batch, false, code);
    });
}
