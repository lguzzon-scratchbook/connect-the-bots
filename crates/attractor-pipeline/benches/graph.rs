use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use attractor_pipeline::PipelineGraph;

fn generate_dot_graph(node_count: usize) -> String {
    let mut dot = format!("digraph Pipeline {{\n");
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

fn generate_branching_dot_graph(branch_factor: usize, depth: usize) -> String {
    let mut dot = format!("digraph Pipeline {{\n");
    dot.push_str("    start [shape=\"Mdiamond\"];\n");
    dot.push_str("    end [shape=\"Msquare\"];\n");

    for d in 0..depth {
        for b in 0..branch_factor.pow(d as u32) {
            let id = format!("node_{}_{}", d, b);
            dot.push_str(&format!("    {} [shape=box];\n", id));
        }
    }

    dot.push_str("    start -> node_0_0;\n");
    for d in 0..depth - 1 {
        for b in 0..branch_factor.pow(d as u32) {
            let parent = format!("node_{}_{}", d, b);
            for i in 0..branch_factor {
                let child = format!("node_{}_{}", d + 1, b * branch_factor + i);
                dot.push_str(&format!("    {} -> {};\n", parent, child));
            }
        }
    }

    for b in 0..branch_factor.pow((depth - 1) as u32) {
        let leaf = format!("node_{}_{}", depth - 1, b);
        dot.push_str(&format!("    {} -> end;\n", leaf));
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
            for i in 0..50 {
                let _ = graph.outgoing_edges(&format!("step{}", i));
            }
        })
    });
}

fn bench_start_node_lookup(c: &mut Criterion) {
    let dot = generate_dot_graph(100);
    let parsed = attractor_dot::parse(&dot).unwrap();
    let graph = PipelineGraph::from_dot(parsed).unwrap();

    c.bench_function("graph/start_node_lookup", |b| {
        b.iter(|| graph.start_node())
    });
}

fn bench_branching_graph(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph/branching");
    for (branches, depth) in [(2, 4), (3, 3), (4, 3)].iter() {
        let dot = generate_branching_dot_graph(*branches, *depth);
        let parsed = attractor_dot::parse(&dot).unwrap();
        let id = format!("{}_branches_{}_depth", branches, depth);
        group.bench_with_input(BenchmarkId::new("from_dot", id), &parsed, |b, parsed| {
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
