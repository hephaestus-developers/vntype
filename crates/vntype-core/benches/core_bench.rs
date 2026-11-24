use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use vntype_core::{
    Composer, SIMPLE_TELEX, TELEX, TransformOptions, VNI, convert_charset, diff, rebuild, to_raw,
    transform_str,
};

fn bench_transform(c: &mut Criterion) {
    let mut group = c.benchmark_group("core/transform");
    let opt = TransformOptions::default();

    for (label, raw) in [
        ("simple_vi", "hoa"),
        ("complex_vi", "nghieng6"),
        ("horn_pair", "tuoi7"),
        ("english_passthrough", "strength"),
    ] {
        group.bench_function(format!("telex_{label}"), |b| {
            b.iter(|| black_box(transform_str(black_box(raw), &TELEX, &opt, true)));
        });
    }

    group.bench_function("simple_telex_complex_vi", |b| {
        b.iter(|| black_box(transform_str(black_box("nghieng6"), &SIMPLE_TELEX, &opt, true)));
    });
    group.bench_function("vni_complex_vi", |b| {
        b.iter(|| black_box(transform_str(black_box("nghieng63"), &VNI, &opt, true)));
    });

    group.bench_function("composer_incremental_word", |b| {
        let keys = ['t', 'i', 'e', 'e', 'n', 'g', 's'];
        b.iter(|| {
            let mut composer = Composer::new(&TELEX, opt);
            for &k in &keys {
                composer.push(black_box(k));
                black_box(composer.result(false));
            }
            black_box(composer.result(true))
        });
    });

    group.finish();
}

fn bench_rebuild_and_diff(c: &mut Criterion) {
    let mut group = c.benchmark_group("core/rebuild_diff");
    let opt = TransformOptions::default();

    group.bench_function("rebuild_telex", |b| {
        b.iter(|| black_box(rebuild(black_box("tiếng"), &TELEX, &opt)));
    });
    group.bench_function("to_raw_telex", |b| {
        b.iter(|| black_box(to_raw(black_box("tiếng"), &TELEX)));
    });
    group.bench_function("diff_mid_word", |b| {
        b.iter(|| black_box(diff(black_box("tiêng"), black_box("tiếng"))));
    });

    group.finish();
}

fn bench_charset(c: &mut Criterion) {
    let mut group = c.benchmark_group("core/charset");
    let short = "Tiếng Việt là ngôn ngữ giàu thanh điệu và sắc thái biểu cảm.";
    let long = short.repeat(120);
    let long_tcvn3 = convert_charset(&long, "unicode", "tcvn3");

    group.bench_function("unicode_to_tcvn3_short", |b| {
        b.iter(|| black_box(convert_charset(black_box(short), "unicode", "tcvn3")));
    });
    group.bench_function("unicode_to_tcvn3_long", |b| {
        b.iter(|| black_box(convert_charset(black_box(&long), "unicode", "tcvn3")));
    });
    group.bench_function("tcvn3_to_unicode_long", |b| {
        b.iter(|| black_box(convert_charset(black_box(&long_tcvn3), "tcvn3", "unicode")));
    });

    group.finish();
}

criterion_group!(benches, bench_transform, bench_rebuild_and_diff, bench_charset);
criterion_main!(benches);
