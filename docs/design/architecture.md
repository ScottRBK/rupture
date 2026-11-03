# Architecture
This workspace will contain:

- a library for agent logic
- a server for exposing agents through an API and MCP
- a CLI application that calls the server to run agents

## Terminology
- Ports - These are Traits what i would have previously defined as Protcols in Python, they express 
what the application needs or exposes.
- Adapters - These are concrete imple─lemtations of these traits
- Services - Business uses cases that depend on the protocols - for this solution which is quite 
small then this is just the `gateway.rs` which describes the domain logic for interacting with LLMs
- Routes - Inbound adapters that translate external input into application calls - for this libary 
crate these would not be utilised here, they would be used in a consuming application.

## ferox
For LLM Inference I use the [ferox](https://github.com/ScottRBK/ferox) crate, which is a separate 
crate that I have [published](https://crates.io/crates/ferox-ai).

Given the fact that the LLM inference is already abstracted, I am not adding an additional port and 
adapter for LLM inference as part of this project. This will pollute slightly the Agent Domain logic
with references to ferox, however prevents the need for an unnecessary duplication of mappings to a 
ruputure specific abstraction of LLM inference.

## Initial Folder Structure

Ports and adapters are added where they represent a real boundary. They are not required in every
crate. The agents crate contains the application logic, while the server routes act as inbound
adapters. The CLI remains a simple server client until it requires additional application logic or
transports.

```text
rupture/
├── Cargo.toml
├── AGENTS.md
├── docs/
│   └── design/
│       └── architecture.md
├── agents/
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs
│   │   ├── agent.rs
│   │   ├── model.rs
│   │   ├── error.rs
│   │   └── adapters/
│   │       ├── mod.rs
│   │       └── ferox.rs
│   └── tests/
│       └── agent.rs
├── server/
│   ├── Cargo.toml
│   ├── src/
│   │   ├── main.rs
│   │   ├── lib.rs
│   │   ├── error.rs
│   │   └── routes/
│   │       ├── mod.rs
│   │       ├── api.rs
│   │       └── mcp.rs
│   └── tests/
│       ├── api.rs
│       └── mcp.rs
└── cli/
    ├── Cargo.toml
    ├── src/
    │   ├── main.rs
    │   ├── lib.rs
    │   ├── app.rs
    │   ├── panel.rs
    │   └── error.rs
    └── tests/
        └── client.rs
```

Additional ports and adapters should be introduced only when an implemented use case needs them. For
example, `ports/tool.rs` should be added when agent tools are first implemented.

