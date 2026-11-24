use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use vntype_syllable::{
    Mark, ToneStyle, check, check_final, compose_char, decompose_char, parse, place_tone,
};

fn bench_parse_and_spellcheck(c: &mut Criterion) {
    let mut group = c.benchmark_group("syllable/parse_spellcheck");

    for (label, word) in [("valid", "nghiêng"), ("partial", "nghi"), ("invalid", "strength")] {
        group.bench_function(format!("parse_{label}"), |b| {
            b.iter(|| black_box(parse(black_box(word))));
        });
        group.bench_function(format!("check_{label}"), |b| {
            b.iter(|| black_box(check(black_box(word), 0)));
        });
    }

    group.bench_function("check_final_valid", |b| {
        b.iter(|| black_box(check_final(black_box("nghiêng"), 0, false)));
    });

    group.finish();
}

fn bench_tone_and_char(c: &mut Criterion) {
    let mut group = c.benchmark_group("syllable/tone_char");

    group.bench_function("place_tone_old", |b| {
        b.iter(|| black_box(place_tone(black_box("hoa"), 2, ToneStyle::Old)));
    });
    group.bench_function("place_tone_new", |b| {
        b.iter(|| black_box(place_tone(black_box("hoa"), 2, ToneStyle::New)));
    });
    group.bench_function("compose_char", |b| {
        b.iter(|| black_box(compose_char(black_box('a'), Some(Mark::Circumflex), 1, false)));
    });
    group.bench_function("decompose_char", |b| {
        b.iter(|| black_box(decompose_char(black_box('ấ'))));
    });

    group.finish();
}

criterion_group!(benches, bench_parse_and_spellcheck, bench_tone_and_char);
criterion_main!(benches);
