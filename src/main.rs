//! Substrate Node Template CLI library.

mod chain_spec;
#[macro_use]
mod service;
mod cli;

pub use substrate_cli::{error, IntoExit, VersionInfo};

fn main() -> Result<(), cli::error::Error> {
	let version = VersionInfo {
		name: "Substrate Node",
		commit: env!("VERGEN_SHA_SHORT"),
		version: env!("CARGO_PKG_VERSION"),
		executable_name: "substrate-poa",
		author: "Anonymous",
		description: "Template Node",
		support_url: "support.anonymous.an",
	};

	cli::run(std::env::args(), cli::Exit, version)
}
