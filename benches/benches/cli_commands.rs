use criterion::{Criterion, black_box, criterion_group, criterion_main};
use std::path::Path;
use std::process::Command;

fn normalize_binary() -> Option<std::path::PathBuf> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.parent().unwrap();
    let bin = workspace_root.join("target/debug/normalize");
    if bin.exists() { Some(bin) } else { None }
}

fn bench_view(c: &mut Criterion) {
    let Some(bin) = normalize_binary() else {
        eprintln!("Skipping view bench: target/debug/normalize not built");
        return;
    };
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.parent().unwrap();
    let target_file = workspace_root.join("crates/normalize-facts/src/index.rs");

    let mut group = c.benchmark_group("cli_commands");
    group.bench_function("normalize view index.rs", |b| {
        b.iter(|| {
            let out = Command::new(&bin)
                .arg("view")
                .arg(&target_file)
                .current_dir(workspace_root)
                .output()
                .expect("run normalize view");
            black_box(out.stdout.len())
        });
    });
    group.finish();
}

fn bench_rank_complexity(c: &mut Criterion) {
    let Some(bin) = normalize_binary() else {
        eprintln!("Skipping rank complexity bench: target/debug/normalize not built");
        return;
    };
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.parent().unwrap();
    let target_dir = workspace_root.join("crates/normalize-facts/src");

    let mut group = c.benchmark_group("cli_commands");
    group.sample_size(10); // CLI startup is slow; fewer samples
    group.bench_function("normalize rank complexity normalize-facts/src", |b| {
        b.iter(|| {
            let out = Command::new(&bin)
                .args(["rank", "complexity"])
                .arg(&target_dir)
                .current_dir(workspace_root)
                .output()
                .expect("run normalize rank complexity");
            black_box(out.stdout.len())
        });
    });
    group.finish();
}

criterion_group!(benches, bench_view, bench_rank_complexity);
criterion_main!(benches);
