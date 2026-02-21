use criterion::{black_box, criterion_group, criterion_main, Criterion};
use soroban_debugger::inspector::{StorageInspector, StorageFilter};
use std::fs;
use tempfile::NamedTempFile;
use std::io::Write;

fn bench_wasm_loading(c: &mut Criterion) {
    let mut file = NamedTempFile::new().unwrap();
    let dummy_wasm = vec![0u8; 100 * 1024]; // 100KB dummy wasm
    file.write_all(&dummy_wasm).unwrap();
    let path = file.path().to_owned();

    c.bench_function("wasm_loading_100kb", |b| {
        b.iter(|| {
            let bytes = fs::read(black_box(&path)).unwrap();
            black_box(bytes);
        })
    });
}

fn bench_storage_snapshot(c: &mut Criterion) {
    let mut inspector = StorageInspector::new();
    for i in 0..1000 {
        inspector.set(format!("key_{}", i), format!("value_{}", i));
    }

    c.bench_function("storage_get_all_1000_entries", |b| {
        b.iter(|| {
            let entries = inspector.get_all();
            black_box(entries);
        })
    });
}

fn bench_storage_diff(c: &mut Criterion) {
    let mut inspector1 = StorageInspector::new();
    let mut inspector2 = StorageInspector::new();
    
    for i in 0..1000 {
        inspector1.set(format!("key_{}", i), format!("value_{}", i));
        if i % 2 == 0 {
            inspector2.set(format!("key_{}", i), format!("value_{}_mod", i));
        } else {
            inspector2.set(format!("key_{}", i), format!("value_{}", i));
        }
    }

    c.bench_function("storage_compare_1000_entries", |b| {
        b.iter(|| {
            let s1 = inspector1.get_all();
            let s2 = inspector2.get_all();
            let mut diff_count = 0;
            for (k, v1) in s1 {
                if let Some(v2) = s2.get(k) {
                    if v1 != v2 {
                        diff_count += 1;
                    }
                }
            }
            black_box(diff_count);
        })
    });
}

fn bench_filter_parsing(c: &mut Criterion) {
    let filters = vec![
        "balance:*".to_string(),
        "re:^user_\\d+$".to_string(),
        "total_supply".to_string(),
    ];

    c.bench_function("storage_filter_parsing_3_patterns", |b| {
        b.iter(|| {
            let filter = StorageFilter::new(black_box(&filters)).unwrap();
            black_box(filter);
        })
    });
}

fn bench_filter_matching(c: &mut Criterion) {
    let filters = vec![
        "balance:*".to_string(),
        "re:^user_\\d+$".to_string(),
        "total_supply".to_string(),
    ];
    let filter = StorageFilter::new(&filters).unwrap();
    let key = "balance:alice";

    c.bench_function("storage_filter_matching", |b| {
        b.iter(|| {
            let result = filter.matches(black_box(key));
            black_box(result);
        })
    });
}

criterion_group!(benches, bench_wasm_loading, bench_storage_snapshot, bench_storage_diff, bench_filter_parsing, bench_filter_matching);
criterion_main!(benches);
