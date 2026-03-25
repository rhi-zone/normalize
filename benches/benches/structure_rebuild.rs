use criterion::{Criterion, black_box, criterion_group, criterion_main};
use normalize_facts::{FileIndex, SymbolParser};
use std::path::Path;
use tokio::runtime::Runtime;

fn bench_symbol_parser(c: &mut Criterion) {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.parent().unwrap();
    let path = workspace_root.join("crates/normalize-facts/src/index.rs");
    let content =
        std::fs::read_to_string(&path).expect("index.rs must exist; run from workspace root");

    let parser = SymbolParser::new();

    let mut group = c.benchmark_group("structure_rebuild/per_file");
    group.bench_function("parse_file(index.rs)", |b| {
        b.iter(|| black_box(parser.parse_file(&path, &content)));
    });
    group.finish();
}

fn bench_file_index_refresh(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.parent().unwrap();
    let src_crate = workspace_root.join("crates/normalize-facts-core");

    // Copy the small crate to a temp dir once; reuse across all iterations
    let tmp = tempfile::TempDir::new().unwrap();
    copy_dir_recursive(&src_crate, tmp.path()).expect("failed to copy crate");

    let mut group = c.benchmark_group("structure_rebuild/full_refresh");
    group.bench_function("FileIndex::refresh(normalize-facts-core)", |b| {
        b.iter(|| {
            rt.block_on(async {
                // Fresh db each iteration (measures full refresh from scratch)
                let db_path = tmp.path().join(format!(
                    "bench-{}.sqlite",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .subsec_nanos()
                ));
                let mut idx = FileIndex::open(&db_path, tmp.path())
                    .await
                    .expect("open index");
                black_box(idx.refresh().await.expect("refresh"))
            })
        });
    });
    group.finish();
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dst_path = dst.join(entry.file_name());
        if ty.is_dir() {
            std::fs::create_dir_all(&dst_path)?;
            copy_dir_recursive(&entry.path(), &dst_path)?;
        } else {
            std::fs::copy(entry.path(), dst_path)?;
        }
    }
    Ok(())
}

criterion_group!(benches, bench_symbol_parser, bench_file_index_refresh);
criterion_main!(benches);
