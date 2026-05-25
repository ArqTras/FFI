//! JSON-line gateway events on stdout for the Flutter desktop sidecar.

use serde_json::Value;

/// Same payload shape as Electron `webContents.send("receive", { event, data })`.
pub trait SoloPoolSink: Send + Sync + 'static {
    fn emit_receive(&self, event: &str, data: Value);
}

/// Flutter spawns `arqma_flutter_solo_pool`; each line is one JSON object `{ "event", "data" }`.
#[derive(Clone, Copy)]
pub struct JsonlStdoutSoloPoolSink;

impl SoloPoolSink for JsonlStdoutSoloPoolSink {
    fn emit_receive(&self, event: &str, data: Value) {
        use std::io::Write;
        let line = serde_json::json!({ "event": event, "data": data }).to_string();
        let mut out = std::io::stdout().lock();
        let _ = writeln!(out, "{line}");
        let _ = out.flush();
    }
}
