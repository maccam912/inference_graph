use criterion::{black_box, criterion_group, criterion_main, Criterion};
use inference_graph::{graph, wrap};

async fn concat(x: Vec<String>) -> String {
    x.concat()
}

pub fn criterion_benchmark(c: &mut Criterion) {
    let mut graph = graph::Graph::default();
    graph.stage_node("A".into(), vec!["entrypoint".into()], wrap!(concat));
    graph.stage_node("B".into(), vec!["entrypoint".into()], wrap!(concat));
    graph.stage_node("C".into(), vec!["A".into(), "B".into()], wrap!(concat));
    let h: String = "hubba".into();
    let n: String = "C".into();
    let tokio_rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    c.bench_function("graph.run", |b| {
        b.iter(|| tokio_rt.block_on(graph.run(black_box(h.clone()), black_box(n.clone()))))
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
