/// React template implementation with pure Rust - no process spawning
/// This module provides functionality to generate React + TypeScript + Apollo Client projects
/// without spawning external processes like `npm create vite`

#[cfg(feature = "react-template")]
pub mod npm_client;

#[cfg(feature = "react-template")]
pub mod template_generator;

#[cfg(feature = "react-template")]
pub mod environment_checker;

#[cfg(feature = "react-template")]
pub mod setup_instructions;

#[cfg(feature = "react-template")]
pub use npm_client::SafeNpmClient;

#[cfg(feature = "react-template")]
pub use template_generator::PureRustViteGenerator;

#[cfg(feature = "react-template")]
pub use environment_checker::SafeEnvironmentChecker;

#[cfg(feature = "react-template")]
pub use setup_instructions::SetupInstructions;