//! Shared parts of the optional Chrome Native Messaging and local inbox.
//! The MCP CLI and Chrome host are separate executables and protocols.
pub mod agent;
pub mod agent_grants;
pub mod agent_health;
pub mod agent_options;
pub mod agent_preferences;
pub mod agent_task;
pub mod chat_copilot;
pub mod chat_source;
pub mod diagnostics;
pub mod ecosystem;
pub mod inbox;
pub mod local_http;
pub mod mcp_provider;
pub mod native;
pub mod native_host_registration;
pub mod pi_enterprise;
pub mod state_file;
pub mod task_center;
pub mod web_v2;
pub mod workbuddy;
pub mod workbuddy_web;
