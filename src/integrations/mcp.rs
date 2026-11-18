use crate::editing::{
    AiPreset, apply_pipeline_with_plugins, list_plugins, parse_operation,
    recommend_copilot_pipeline,
};
use crate::integrations::video::{VideoEditOptions, run_video_edit};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
struct RpcRequest {
    method: String,
    #[serde(default)]
    id: Option<Value>,
    #[serde(default)]
    params: Value,
}

#[derive(Debug, Serialize)]
struct RpcResponse {
    jsonrpc: &'static str,
    id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<RpcError>,
}

#[derive(Debug, Serialize)]
struct RpcError {
    code: i32,
    message: String,
}

fn rpc_ok(id: Option<Value>, result: Value) -> RpcResponse {
    RpcResponse {
        jsonrpc: "2.0",
        id,
        result: Some(result),
        error: None,
    }
}

fn rpc_err(id: Option<Value>, code: i32, message: impl Into<String>) -> RpcResponse {
    RpcResponse {
        jsonrpc: "2.0",
        id,
        result: None,
        error: Some(RpcError {
            code,
            message: message.into(),
        }),
    }
}

fn get_tool_name(params: &Value) -> Option<&str> {
    params
        .get("name")
        .and_then(Value::as_str)
        .or_else(|| params.get("tool").and_then(Value::as_str))
}

fn handle_rpc(req: RpcRequest) -> RpcResponse {
    match req.method.as_str() {
        "initialize" => rpc_ok(
            req.id,
            json!({
                "serverInfo": { "name": "forma-mcp", "version": env!("CARGO_PKG_VERSION") },
                "capabilities": { "tools": {} }
            }),
        ),
        "tools/list" => rpc_ok(
            req.id,
            json!({
                "tools": [
                    {
                        "name": "apply_pipeline",
                        "description": "Apply photo editing ops to an image (supports PSD input and RAW fallback)",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "input": { "type": "string" },
                                "output": { "type": "string" },
                                "ops": { "type": "array", "items": { "type": "string" } },
                                "ai": { "type": "string" },
                                "copilot_suggest": { "type": "boolean" },
                                "gpu": { "type": "boolean" },
                                "plugins": { "type": "array", "items": { "type": "string" } }
                            },
                            "required": ["input", "output"]
                        }
                    },
                    {
                        "name": "recommend_pipeline",
                        "description": "Get Copilot-style suggested operations for an image",
                        "inputSchema": {
                            "type": "object",
                            "properties": { "input": { "type": "string" } },
                            "required": ["input"]
                        }
                    },
                    {
                        "name": "list_plugins",
                        "description": "List discovered JavaScript image plugins",
                        "inputSchema": { "type": "object", "properties": {} }
                    },
                    {
                        "name": "video_edit",
                        "description": "Run advanced ffmpeg video editing pipeline",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "input": { "type": "string" },
                                "output": { "type": "string" },
                                "trim_start": { "type": "string" },
                                "trim_end": { "type": "string" },
                                "resize": { "type": "string" },
                                "fps": { "type": "number" },
                                "speed": { "type": "number" },
                                "mute": { "type": "boolean" },
                                "overlay_image": { "type": "string" },
                                "overlay_pos": { "type": "string" },
                                "color_grade": { "type": "string" },
                                "codec": { "type": "string" },
                                "crf": { "type": "number" },
                                "preset": { "type": "string" }
                            },
                            "required": ["input", "output"]
                        }
                    }
                ]
            }),
        ),
        "tools/call" => {
            let name = match get_tool_name(&req.params) {
                Some(v) => v,
                None => return rpc_err(req.id, -32602, "missing tool name"),
            };
            let args = req.params.get("arguments").unwrap_or(&req.params);
            match name {
                "apply_pipeline" => {
                    let input = match args.get("input").and_then(Value::as_str) {
                        Some(v) => PathBuf::from(v),
                        None => return rpc_err(req.id, -32602, "missing input"),
                    };
                    let output = match args.get("output").and_then(Value::as_str) {
                        Some(v) => PathBuf::from(v),
                        None => return rpc_err(req.id, -32602, "missing output"),
                    };
                    let op_strings = args
                        .get("ops")
                        .and_then(Value::as_array)
                        .map(|arr| {
                            arr.iter()
                                .filter_map(Value::as_str)
                                .map(ToString::to_string)
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();
                    let ops = match op_strings
                        .iter()
                        .map(|v| parse_operation(v))
                        .collect::<Result<Vec<_>>>()
                    {
                        Ok(v) => v,
                        Err(err) => {
                            return rpc_err(req.id, -32602, format!("invalid ops: {err:#}"));
                        }
                    };
                    let ai = match args.get("ai").and_then(Value::as_str) {
                        Some(v) => match AiPreset::parse(v) {
                            Ok(p) => Some(p),
                            Err(err) => return rpc_err(req.id, -32602, err.to_string()),
                        },
                        None => None,
                    };
                    let copilot_suggest = args
                        .get("copilot_suggest")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    let gpu = args.get("gpu").and_then(Value::as_bool).unwrap_or(false);
                    let plugins = args
                        .get("plugins")
                        .and_then(Value::as_array)
                        .map(|items| {
                            items
                                .iter()
                                .filter_map(Value::as_str)
                                .map(ToString::to_string)
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();

                    match apply_pipeline_with_plugins(
                        &input,
                        &output,
                        &ops,
                        ai,
                        copilot_suggest,
                        gpu,
                        &plugins,
                        &[],
                    ) {
                        Ok(effective) => rpc_ok(
                            req.id,
                            json!({
                                "output": output.to_string_lossy(),
                                "operations": effective.iter().map(ToString::to_string).collect::<Vec<_>>(),
                                "plugins": plugins
                            }),
                        ),
                        Err(err) => rpc_err(req.id, -32000, format!("{err:#}")),
                    }
                }
                "recommend_pipeline" => {
                    let input = match args.get("input").and_then(Value::as_str) {
                        Some(v) => PathBuf::from(v),
                        None => return rpc_err(req.id, -32602, "missing input"),
                    };
                    match crate::editing::load_image_with_psd_and_raw_support(&input)
                        .map(|img| recommend_copilot_pipeline(&img))
                    {
                        Ok(ops) => rpc_ok(
                            req.id,
                            json!({ "ops": ops.iter().map(ToString::to_string).collect::<Vec<_>>() }),
                        ),
                        Err(err) => rpc_err(req.id, -32000, format!("{err:#}")),
                    }
                }
                "list_plugins" => match list_plugins(&[]) {
                    Ok(plugins) => rpc_ok(
                        req.id,
                        json!({
                            "plugins": plugins.iter().map(|plugin| json!({
                                "id": plugin.id,
                                "name": plugin.name,
                                "version": plugin.version,
                                "path": plugin.path.to_string_lossy()
                            })).collect::<Vec<_>>()
                        }),
                    ),
                    Err(err) => rpc_err(req.id, -32000, format!("{err:#}")),
                },
                "video_edit" => {
                    let input = match args.get("input").and_then(Value::as_str) {
                        Some(v) => PathBuf::from(v),
                        None => return rpc_err(req.id, -32602, "missing input"),
                    };
                    let output = match args.get("output").and_then(Value::as_str) {
                        Some(v) => PathBuf::from(v),
                        None => return rpc_err(req.id, -32602, "missing output"),
                    };
                    let fps = args
                        .get("fps")
                        .and_then(Value::as_u64)
                        .and_then(|v| u32::try_from(v).ok());
                    let crf = args
                        .get("crf")
                        .and_then(Value::as_u64)
                        .and_then(|v| u8::try_from(v).ok())
                        .unwrap_or(20);
                    let options = VideoEditOptions {
                        input,
                        output,
                        trim_start: args
                            .get("trim_start")
                            .and_then(Value::as_str)
                            .map(ToString::to_string),
                        trim_end: args
                            .get("trim_end")
                            .and_then(Value::as_str)
                            .map(ToString::to_string),
                        resize: args
                            .get("resize")
                            .and_then(Value::as_str)
                            .map(ToString::to_string),
                        fps,
                        speed: args.get("speed").and_then(Value::as_f64).map(|v| v as f32),
                        mute: args.get("mute").and_then(Value::as_bool).unwrap_or(false),
                        overlay_image: args
                            .get("overlay_image")
                            .and_then(Value::as_str)
                            .map(PathBuf::from),
                        overlay_pos: args
                            .get("overlay_pos")
                            .and_then(Value::as_str)
                            .unwrap_or("20:20")
                            .to_string(),
                        color_grade: args
                            .get("color_grade")
                            .and_then(Value::as_str)
                            .map(ToString::to_string),
                        codec: args
                            .get("codec")
                            .and_then(Value::as_str)
                            .unwrap_or("libx264")
                            .to_string(),
                        crf,
                        preset: args
                            .get("preset")
                            .and_then(Value::as_str)
                            .unwrap_or("medium")
                            .to_string(),
                    };
                    match run_video_edit(options) {
                        Ok(_) => rpc_ok(req.id, json!({"ok": true})),
                        Err(err) => rpc_err(req.id, -32000, format!("{err:#}")),
                    }
                }
                _ => rpc_err(req.id, -32601, format!("unknown tool '{name}'")),
            }
        }
        _ => rpc_err(req.id, -32601, format!("unknown method '{}'", req.method)),
    }
}

pub fn run_mcp_server() -> Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let parsed = serde_json::from_str::<RpcRequest>(&line);
        let response = match parsed {
            Ok(req) => handle_rpc(req),
            Err(err) => rpc_err(None, -32700, format!("parse error: {err}")),
        };
        serde_json::to_writer(&mut stdout, &response)?;
        stdout.write_all(b"\n")?;
        stdout.flush()?;
    }
    Ok(())
}
