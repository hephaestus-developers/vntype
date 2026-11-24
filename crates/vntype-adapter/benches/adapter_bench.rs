use std::hint::black_box;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use vntype_adapter::{AdapterController, char_to_utf16_offset, utf16_to_byte_offset};
use vntype_engine::{Config, KeyEvent, OutputMode};

fn bench_controller(c: &mut Criterion) {
    let mut group = c.benchmark_group("adapter/controller");
    let events: Vec<KeyEvent> = "tieengs ".chars().map(KeyEvent::ch).collect();

    group.bench_function("process_word_preedit", |b| {
        b.iter_batched(
            || AdapterController::new(Config::default(), OutputMode::Preedit),
            |mut ctrl| {
                for ev in &events {
                    black_box(ctrl.process_key(black_box(ev.clone()), 0));
                }
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

fn bench_offset(c: &mut Criterion) {
    let mut group = c.benchmark_group("adapter/offset");
    let text = "Tiếng Việt có dấu thanh và chữ ghép rất đa dạng. ".repeat(20);

    group.bench_function("utf16_to_byte_1kb", |b| {
        b.iter(|| black_box(utf16_to_byte_offset(black_box(&text), 400)));
    });
    group.bench_function("char_to_utf16_1kb", |b| {
        b.iter(|| black_box(char_to_utf16_offset(black_box(&text), 400)));
    });

    group.finish();
}

criterion_group!(benches, bench_controller, bench_offset);
criterion_main!(benches);
