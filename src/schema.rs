use serde_json::{Value, json};

pub fn schema_json() -> Value {
    let runtime_values = json!(["headless", "mirrored", "tui"]);
    json!({
        "name": "swarmux",
        "agent_first": true,
        "commands": [
            {
                "name": "schema",
                "json_input": false,
                "mutating": false
            },
            {
                "name": "doctor",
                "json_input": false,
                "mutating": false
            },
            {
                "name": "init",
                "json_input": false,
                "mutating": true
            },
            {
                "name": "paths",
                "json_input": false,
                "mutating": false
            },
            {
                "name": "submit",
                "json_input": true,
                "mutating": true,
                "supports_dry_run": true,
                "runtime_field": "runtime",
                "runtime_values": runtime_values,
                "required_payload_fields": [
                    "title",
                    "repo_ref",
                    "repo_root",
                    "mode",
                    "command"
                ]
            },
            {
                "name": "delegate",
                "json_input": true,
                "mutating": true,
                "supports_dry_run": true,
                "runtime_field": "runtime",
                "runtime_values": runtime_values
            },
            {
                "name": "dispatch",
                "json_input": false,
                "mutating": true,
                "supports_dry_run": true,
                "runtime_flag": "--runtime",
                "runtime_values": runtime_values
            },
            {
                "name": "start",
                "json_input": false,
                "mutating": true
            },
            {
                "name": "list",
                "json_input": false,
                "mutating": false
            },
            {
                "name": "show",
                "json_input": false,
                "mutating": false
            },
            {
                "name": "logs",
                "json_input": false,
                "mutating": false
            },
            {
                "name": "notify",
                "json_input": false,
                "mutating": true
            },
            {
                "name": "watch",
                "json_input": false,
                "mutating": true
            },
            {
                "name": "send",
                "json_input": false,
                "mutating": true
            },
            {
                "name": "attach",
                "json_input": false,
                "mutating": false
            },
            {
                "name": "stop",
                "json_input": false,
                "mutating": true
            },
            {
                "name": "done",
                "json_input": false,
                "mutating": true
            },
            {
                "name": "fail",
                "json_input": false,
                "mutating": true
            },
            {
                "name": "reconcile",
                "json_input": false,
                "mutating": true
            },
            {
                "name": "prune",
                "json_input": false,
                "mutating": true
            },
            {
                "name": "overview",
                "json_input": false,
                "mutating": false
            }
        ]
    })
}
