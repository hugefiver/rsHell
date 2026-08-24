use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use rshell_core::{ResolvedTerminalProfile, TerminalOverrides, TerminalSettingsV1, TerminalSize};
use rshell_session::DefaultTerminalEngine;

fn profile() -> ResolvedTerminalProfile {
    TerminalSettingsV1::default().resolve(&TerminalOverrides::default())
}

fn benchmark_throughput(criterion: &mut Criterion) {
    let chunk = b"plain benchmark payload 0123456789\x1b[32mGREEN\x1b[0m\r\n";
    let mut workload = Vec::with_capacity(1024 * 1024);
    while workload.len() < 1024 * 1024 {
        workload.extend_from_slice(chunk);
    }
    let size = TerminalSize {
        cols: 120,
        rows: 36,
        pixel_width: 960,
        pixel_height: 576,
        dpi: 96,
    };
    let mut group = criterion.benchmark_group("terminal_engine");
    group.throughput(Throughput::Bytes(workload.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("input_mixed_ansi", workload.len()),
        &workload,
        |bencher, bytes| {
            let mut engine = DefaultTerminalEngine::new(&profile(), size).unwrap();
            bencher.iter(|| engine.input(bytes).unwrap());
        },
    );
    group.finish();
}

criterion_group!(benches, benchmark_throughput);
criterion_main!(benches);
