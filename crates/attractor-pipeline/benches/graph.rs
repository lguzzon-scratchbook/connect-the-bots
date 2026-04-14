use attractor_pipeline::PipelineGraph;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

fn generate_dot_graph(node_count: usize) -> String {
    let mut dot = "digraph Pipeline {\n".to_string();
    dot.push_str("    start [shape=\"Mdiamond\",label=\"Start\"];\n");
    dot.push_str("    done [shape=\"Msquare\",label=\"Done\"];\n");
    for i in 0..node_count {
        dot.push_str(&format!(
            "    step{} [shape=\"box\",label=\"Step {}\",max_retries=\"3\"];\n",
            i, i
        ));
    }
    dot.push_str("    start -> step0;\n");
    for i in 0..node_count - 1 {
        dot.push_str(&format!("    step{} -> step{};\n", i, i + 1));
    }
    dot.push_str(&format!("    step{} -> done;\n", node_count - 1));
    dot.push_str("}\n");
    dot
}

fn generate_branching_dot_graph(node_count: usize) -> String {
    // Generate a simple valid DOT graph for branching benchmark
    // Creates a binary tree structure with proper quoted attributes
    let mut dot = "digraph Pipeline {\n".to_string();
    dot.push_str("    start [shape=\"Mdiamond\"];\n");
    dot.push_str("    end [shape=\"Msquare\"];\n");

    // Create nodes with properly quoted attributes
    for i in 0..node_count {
        dot.push_str(&format!("    n{} [shape=\"box\"];\n", i));
    }

    // Connect start to first node
    if node_count > 0 {
        dot.push_str("    start -> n0;\n");
    }

    // Create binary tree edges
    for i in 0..node_count {
        let left = i * 2 + 1;
        let right = i * 2 + 2;
        if left < node_count {
            dot.push_str(&format!("    n{} -> n{};\n", i, left));
        }
        if right < node_count {
            dot.push_str(&format!("    n{} -> n{};\n", i, right));
        }
        // Connect leaf nodes to end
        if left >= node_count && right >= node_count {
            dot.push_str(&format!("    n{} -> end;\n", i));
        }
    }

    dot.push_str("}\n");
    dot
}

fn bench_graph_from_dot_small(c: &mut Criterion) {
    let input = "digraph Test { start -> A -> B -> done }";
    let parsed = attractor_dot::parse(input).unwrap();

    c.bench_function("graph/from_dot_small_4_nodes", |b| {
        b.iter(|| PipelineGraph::from_dot(black_box(parsed.clone())).unwrap())
    });
}

fn bench_graph_from_dot_large(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph/from_dot_large");
    for size in [10, 50, 100].iter() {
        let dot = generate_dot_graph(*size);
        let parsed = attractor_dot::parse(&dot).unwrap();
        group.bench_with_input(BenchmarkId::from_parameter(size), &parsed, |b, parsed| {
            b.iter(|| PipelineGraph::from_dot(black_box(parsed.clone())).unwrap())
        });
    }
    group.finish();
}

fn bench_outgoing_edges(c: &mut Criterion) {
    let dot = generate_dot_graph(50);
    let parsed = attractor_dot::parse(&dot).unwrap();
    let graph = PipelineGraph::from_dot(parsed).unwrap();

    c.bench_function("graph/outgoing_edges_lookup", |b| {
        b.iter(|| {
            // Batch 50 lookups to amortize measurement overhead
            for i in 0..50 {
                black_box(graph.outgoing_edges(&format!("step{}", i)));
            }
        })
    });
}

fn bench_start_node_lookup(c: &mut Criterion) {
    let dot = generate_dot_graph(100);
    let parsed = attractor_dot::parse(&dot).unwrap();
    let graph = PipelineGraph::from_dot(parsed).unwrap();

    c.bench_function("graph/start_node_lookup", |b| {
        b.iter(|| black_box(graph.start_node()))
    });
}

fn bench_branching_graph(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph/branching");
    for size in [7, 15, 31].iter() {
        let dot = generate_branching_dot_graph(*size);
        let parsed = attractor_dot::parse(&dot).unwrap();
        group.bench_with_input(BenchmarkId::from_parameter(size), &parsed, |b, parsed| {
            b.iter(|| PipelineGraph::from_dot(black_box(parsed.clone())).unwrap())
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_graph_from_dot_small,
    bench_graph_from_dot_large,
    bench_outgoing_edges,
    bench_start_node_lookup,
    bench_branching_graph
);
criterion_main!(benches);
