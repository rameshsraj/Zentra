pub mod discovery;
pub mod leader_election;
pub mod orchestrator;
pub mod plugin_manager;
pub mod streaming;

pub mod collectors {
    pub mod hubble;
    pub mod ebpf_profiler;
}
