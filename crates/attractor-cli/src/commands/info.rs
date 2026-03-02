use anyhow;

pub fn cmd_info(path: &std::path::Path) -> anyhow::Result<()> {
    let graph = crate::load_pipeline(path)?;

    println!("Pipeline: {}", graph.name);
    if !graph.goal.is_empty() {
        println!("Goal: {}", graph.goal);
    }

    let node_count = graph.all_nodes().count();
    let edge_count = graph.all_edges().len();
    println!("Nodes: {}", node_count);
    println!("Edges: {}", edge_count);

    if let Some(start) = graph.start_node() {
        println!("Start: {} ({})", start.id, start.label);
    }
    if let Some(exit) = graph.exit_node() {
        println!("Exit: {} ({})", exit.id, exit.label);
    }

    // List nodes with their types
    println!("\nNodes:");
    for node in graph.all_nodes() {
        let node_type = node.node_type.as_deref().unwrap_or("(default)");
        println!(
            "  {} [{}] shape={} type={}",
            node.id, node.label, node.shape, node_type
        );
    }

    Ok(())
}
