use novel_studio::{
    project::{Project, Result},
    tasks::{completed, head, NewProposal},
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    io::{self, BufRead, Write},
    path::PathBuf,
};

fn main() {
    if let Err(error) = serve() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn serve() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() != Some(std::ffi::OsStr::new("--project")) {
        return Err("Usage: novel-studio-mcp --project <directory>".into());
    }
    let root = args
        .next()
        .map(PathBuf::from)
        .ok_or("Missing project directory.")?;
    if args.next().is_some() {
        return Err("Usage: novel-studio-mcp --project <directory>".into());
    }
    let project = Project::open(&root)?;
    let mut output = io::stdout().lock();
    for line in io::stdin().lock().lines() {
        let line = line.map_err(|e| e.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(message) => respond(&project, &message),
            Err(_) => Some(rpc_error(Value::Null, -32700, "Parse error")),
        };
        if let Some(reply) = reply {
            writeln!(output, "{reply}")
                .and_then(|_| output.flush())
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn respond(project: &Project, message: &Value) -> Option<Value> {
    let id = message.get("id").cloned();
    if message["jsonrpc"] != "2.0"
        || !message["method"].is_string()
        || id
            .as_ref()
            .is_some_and(|id| !id.is_string() && !id.is_i64() && !id.is_u64())
    {
        return Some(rpc_error(Value::Null, -32600, "Invalid Request"));
    }
    let id = id?;
    let method = message["method"].as_str()?;
    let params = &message["params"];
    let result = match method {
        "initialize" => {
            let Some(requested) = params["protocolVersion"].as_str() else {
                return Some(rpc_error(id, -32602, "Missing protocolVersion"));
            };
            let version = match requested {
                "2024-11-05" | "2025-03-26" | "2025-06-18" | "2025-11-25" => requested,
                _ => "2025-11-25",
            };
            json!({
                "protocolVersion": version,
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "novel-studio", "version": env!("CARGO_PKG_VERSION")},
                "instructions": "Use list_tasks to find author assignments, then get_task_context. Use read_document to obtain the current version and exact text before creating a proposal. Proposals are reviewed and applied by the author in Novel Studio."
            })
        }
        "ping" => json!({}),
        "tools/list" => json!({"tools": tool_definitions()}),
        "tools/call" => {
            let Some(name) = params["name"].as_str() else {
                return Some(rpc_error(id, -32602, "Missing tool name"));
            };
            let defaults = json!({});
            let arguments = params.get("arguments").unwrap_or(&defaults);
            match call(project, name, arguments) {
                Ok(value) => json!({
                    "content": [{"type": "text", "text": value.to_string()}],
                    "structuredContent": value
                }),
                Err(error) => {
                    json!({"content": [{"type": "text", "text": error}], "isError": true})
                }
            }
        }
        _ => return Some(rpc_error(id, -32601, "Method not found")),
    };
    Some(json!({"jsonrpc": "2.0", "id": id, "result": result}))
}

fn rpc_error(id: Value, code: i32, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

fn tool_definitions() -> Value {
    json!([
        {
            "name": "list_tasks", "description": "List active author assignments, including tasks with proposals awaiting review.",
            "inputSchema": {"type": "object", "properties": {
                "offset": {"type": "integer", "minimum": 0, "description": "Start at 0. Each page returns at most 30 assignments."}
            }, "additionalProperties": false}
        },
        {
            "name": "get_task_context", "description": "Read the exact assignment, current selection location (null when unresolved), nearby manuscript text and project references.",
            "inputSchema": {"type": "object", "properties": {"taskId": {"type": "string"}}, "required": ["taskId"], "additionalProperties": false}
        },
        {
            "name": "search_project", "description": "Search project Markdown by literal text. Returns up to 30 path and line matches.",
            "inputSchema": {"type": "object", "properties": {"query": {"type": "string"}}, "required": ["query"], "additionalProperties": false}
        },
        {
            "name": "read_document", "description": "Read project Markdown with its saved version for proposals. Lines are 1-based.",
            "inputSchema": {"type": "object", "properties": {
                "path": {"type": "string", "description": "Project-relative Markdown path using /."},
                "startLine": {"type": "integer", "minimum": 1},
                "limit": {"type": "integer", "minimum": 1, "maximum": 300}
            }, "required": ["path"], "additionalProperties": false}
        },
        {
            "name": "submit_proposal", "description": "Send a proposed change for author review. Allow one exact, unique replacement per existing Markdown file. An empty changes array submits findings without text changes.",
            "inputSchema": {"type": "object", "properties": {
                "taskId": {"type": "string"},
                "summary": {"type": "string", "description": "Explain the proposed changes and any material requiring review."},
                "changes": {"type": "array", "maxItems": 20, "items": {
                    "type": "object", "properties": {
                        "path": {"type": "string"},
                        "version": {"type": "string", "description": "Saved file version from read_document."},
                        "oldText": {"type": "string", "minLength": 1, "description": "Exact, unique passage from the saved file."},
                        "newText": {"type": "string"}
                    }, "required": ["path", "version", "oldText", "newText"], "additionalProperties": false
                }}
            }, "required": ["taskId", "summary", "changes"], "additionalProperties": false}
        }
    ])
}

fn call(project: &Project, name: &str, arguments: &Value) -> Result<Value> {
    let string = |key: &str| {
        arguments[key]
            .as_str()
            .ok_or_else(|| format!("Missing or invalid {key}."))
    };
    let result = match name {
        "list_tasks" => {
            let offset = arguments
                .get("offset")
                .map_or(Some(0), Value::as_u64)
                .and_then(|n| usize::try_from(n).ok())
                .ok_or("Invalid offset.")?;
            let tasks = project.tasks()?;
            let mut active = tasks
                .iter()
                .rev()
                .filter(|task| !completed(task))
                .skip(offset);
            let items = active.by_ref().take(30).map(|task| json!({
                "id": task.id, "path": task.path, "instruction": head(&task.instruction, 600),
                "proposalCount": task.proposals.len()
            })).collect::<Vec<_>>();
            json!({"tasks": items, "nextOffset": active.next().map(|_| offset.saturating_add(items.len()))})
        }
        "get_task_context" => json!(project.context(string("taskId")?)?),
        "search_project" => json!({"matches": project.search(string("query")?)?}),
        "read_document" => {
            let page: Page =
                serde_json::from_value(arguments.clone()).map_err(|e| e.to_string())?;
            if page.start_line == Some(0)
                || page.limit.is_some_and(|limit| !(1..=300).contains(&limit))
            {
                return Err("Invalid line range.".into());
            }
            let document = project.read(&page.path)?;
            let start = page.start_line.unwrap_or(1);
            let mut lines = document
                .text
                .split_inclusive('\n')
                .skip(start - 1)
                .peekable();
            let mut text = String::new();
            let mut consumed = 0;
            for line in lines.by_ref().take(page.limit.unwrap_or(100)) {
                text.push_str(line);
                consumed += 1;
                if text.len() >= 48_000 {
                    break;
                }
            }
            json!({"path": document.path, "version": document.version, "startLine": start,
                "text": text, "nextLine": lines.peek().map(|_| start + consumed)})
        }
        "submit_proposal" => {
            let input: NewProposal =
                serde_json::from_value(arguments.clone()).map_err(|e| e.to_string())?;
            let proposal = project.submit(input)?;
            json!({"id": proposal.id, "status": proposal.status})
        }
        _ => return Err(format!("Unknown tool: {name}")),
    };
    Ok(result)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Page {
    path: String,
    start_line: Option<usize>,
    limit: Option<usize>,
}
