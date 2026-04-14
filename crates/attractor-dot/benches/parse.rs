use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};

fn generate_large_graph(node_count: usize) -> String {
    // Pre-allocate capacity to minimize reallocations (rough estimate: ~80 bytes per node)
    let mut dot = String::with_capacity(node_count * 80 + 50);
    dot.push_str("digraph Large {\n");
    for i in 0..node_count {
        dot.push_str(&format!(
            "    node{} [shape=\"box\",label=\"Node {}\",max_retries=\"3\",timeout=\"30s\"];\n",
            i, i
        ));
    }
    for i in 0..node_count - 1 {
        dot.push_str(&format!("    node{} -> node{};\n", i, i + 1));
    }
    dot.push_str("}\n");
    dot
}

fn generate_subgraph_graph(nested_count: usize) -> String {
    let mut dot = format!("digraph Nested {{\n");
    dot.push_str("    start [shape=\"Mdiamond\"];\n");
    dot.push_str("    end [shape=\"Msquare\"];\n");
    for i in 0..nested_count {
        dot.push_str(&format!(
            "    subgraph cluster_{} {{\n        inner{}_a -> inner{}_b;\n    }}\n",
            i, i, i
        ));
    }
    dot.push_str("    start -> end;\n");
    dot.push_str("}\n");
    dot
}

fn bench_parse_simple(c: &mut Criterion) {
    let input = "digraph Test { A -> B -> C }";
    c.bench_function("parse/simple_linear_3_nodes", |b| {
        b.iter(|| attractor_dot::parse(black_box(input)).expect("benchmark input is valid DOT"))
    });
}

fn bench_parse_with_attributes(c: &mut Criterion) {
    let input = r#"digraph G {
        start [shape="Mdiamond", label="Begin", max_retries=3, timeout=30s, goal_gate=true]
        process [shape="box", label="Process", prompt="Do work", fidelity="full"]
        done [shape="Msquare", label="End"]
        start -> process -> done
    }"#;
    c.bench_function("parse/with_attributes", |b| {
        b.iter(|| attractor_dot::parse(black_box(input)).expect("benchmark input is valid DOT"))
    });
}

fn bench_parse_large_graphs(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse/large_graphs");
    for size in [10, 50, 100, 200].iter() {
        let input = generate_large_graph(*size);
        group.bench_with_input(BenchmarkId::from_parameter(size), &input, |b, input| {
            b.iter(|| attractor_dot::parse(black_box(input)).expect("benchmark input is valid DOT"))
        });
    }
    group.finish();
}

fn bench_parse_subgraphs(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse/subgraphs");
    for count in [5, 10, 20].iter() {
        let input = generate_subgraph_graph(*count);
        group.bench_with_input(BenchmarkId::from_parameter(count), &input, |b, input| {
            b.iter(|| attractor_dot::parse(black_box(input)).expect("benchmark input is valid DOT"))
        });
    }
    group.finish();
}

fn bench_parse_chained_edges(c: &mut Criterion) {
    // Test edge chain expansion: A -> B -> C -> D [label="chain"]
    let input = r#"digraph G {
        A -> B -> C -> D -> E -> F -> G -> H [label="chain", weight=10]
    }"#;
    c.bench_function("parse/chained_edges_8", |b| {
        b.iter(|| attractor_dot::parse(black_box(input)).expect("benchmark input is valid DOT"))
    });
}

criterion_group!(
    benches,
    bench_parse_simple,
    bench_parse_with_attributes,
    bench_parse_large_graphs,
    bench_parse_subgraphs,
    bench_parse_chained_edges
);
criterion_main!(benches);
