use std::collections::HashMap;
use std::hint::black_box;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use vntype_engine::{
    Dictionary, ImeSession, KeyEvent, MemoryConfigStore, OutputMode, Ports, StaticAppContext,
    SurroundingText, WordListDictionary, expand_single_macro, match_phrase_macro, shared_dictionary,
};

struct NoSurrounding;

impl SurroundingText for NoSurrounding {
    fn before_cursor(&self, _n: usize) -> Option<String> {
        None
    }
}

fn make_session() -> ImeSession {
    let ports = Ports {
        config: Box::new(MemoryConfigStore::default()),
        app: Box::new(StaticAppContext::new("bench")),
        surrounding: Box::new(NoSurrounding),
        dictionary: Box::new(shared_dictionary()),
    };
    ImeSession::new(ports, OutputMode::Preedit)
}

fn bench_session_typing(c: &mut Criterion) {
    let mut group = c.benchmark_group("engine/session");
    let events: Vec<KeyEvent> =
        "tooi ddanwg gox tieengs vieetj treen linux ".chars().map(KeyEvent::ch).collect();

    group.bench_function("process_sentence_42_keys", |b| {
        b.iter_batched(
            make_session,
            |mut session| {
                for ev in &events {
                    black_box(session.process_key(black_box(ev)));
                }
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

fn bench_dictionary(c: &mut Criterion) {
    let mut group = c.benchmark_group("engine/dictionary");
    let shared = shared_dictionary();

    group.bench_function("shared_hit_vi", |b| {
        b.iter(|| black_box(shared.is_vietnamese(black_box("việt"))));
    });
    group.bench_function("shared_hit_en", |b| {
        b.iter(|| black_box(shared.is_english(black_box("hello"))));
    });
    group.bench_function("shared_miss", |b| {
        b.iter(|| black_box(shared.is_vietnamese(black_box("zzzzzz"))));
    });

    for size in [1_000usize, 10_000, 50_000] {
        let mut dict = WordListDictionary::from_text("", "");
        let words: Vec<String> = (0..size).map(|i| format!("tu{i}")).collect();
        dict.add_words(words, true);
        let probe = format!("tu{}", size / 2);

        group.bench_function(format!("lookup_{size}"), |b| {
            b.iter(|| black_box(dict.is_vietnamese(black_box(&probe))));
        });
    }

    group.finish();
}

fn build_macros(count: usize) -> HashMap<String, String> {
    (0..count).map(|i| (format!("m{i}"), format!("mở rộng {i}"))).collect()
}

fn bench_macros(c: &mut Criterion) {
    let mut group = c.benchmark_group("engine/macro");

    for size in [20usize, 1_000] {
        let macros = build_macros(size);
        let target_key = format!("m{}", size / 2);
        let phrase = format!("xin chào {target_key}");

        group.bench_function(format!("expand_single_{size}"), |b| {
            b.iter(|| {
                black_box(expand_single_macro(&macros, black_box(&target_key), black_box(&target_key)))
            });
        });
        group.bench_function(format!("match_phrase_{size}"), |b| {
            b.iter(|| black_box(match_phrase_macro(&macros, black_box(&phrase), 3)));
        });
    }

    group.finish();
}

criterion_group!(benches, bench_session_typing, bench_dictionary, bench_macros);
criterion_main!(benches);
