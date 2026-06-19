pub mod discovery;
pub mod leader_election;
pub mod orchestrator;
pub mod plugin_manager;
pub mod streaming;
pub mod remote_delivery;
pub mod offline_buffer;
pub mod heartbeat;
pub mod remote_targets;

pub mod collectors {
    pub mod hubble;
    pub mod ebpf_profiler;
}
