//! Shared parts of the optional Chrome Native Messaging and local inbox.
//! The MCP CLI and Chrome host are separate executables and protocols.
pub mod agent;
pub mod agent_grants;
pub mod agent_task;
pub mod agent_options;
pub mod inbox;
pub mod local_http;
pub mod mcp_provider;
pub mod native;
pub mod pi_enterprise;
pub mod web_v2;
pub mod workbuddy;
pub mod workbuddy_web;
