use sysinfo::Networks;

/// Cumulative (rx_total, tx_total) per interface. The sampler turns these into rates.
pub fn counters(networks: &Networks) -> Vec<(String, u64, u64)> {
    networks
        .iter()
        .map(|(name, data)| (name.clone(), data.total_received(), data.total_transmitted()))
        .collect()
}
