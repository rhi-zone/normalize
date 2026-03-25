use criterion::{Criterion, black_box, criterion_group, criterion_main};
use normalize_rules::build_relations_from_index;
use std::path::Path;
use tokio::runtime::Runtime;

fn bench_build_relations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    // Use the normalize repo itself as real-world data
    // Walk up from CARGO_MANIFEST_DIR to find the workspace root
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.parent().unwrap();
    let index_path = workspace_root.join(".normalize/index.sqlite");

    if !index_path.exists() {
        eprintln!(
            "Skipping rules_runner benchmarks: no index at {}\n\
             Run `normalize structure rebuild` first.",
            index_path.display()
        );
        return;
    }

    let mut group = c.benchmark_group("rules_runner");
    group.bench_function("build_relations_from_index(normalize)", |b| {
        b.iter(|| {
            rt.block_on(async {
                black_box(
                    build_relations_from_index(workspace_root)
                        .await
                        .expect("build relations"),
                )
            })
        });
    });
    group.finish();
}

criterion_group!(benches, bench_build_relations);
criterion_main!(benches);
