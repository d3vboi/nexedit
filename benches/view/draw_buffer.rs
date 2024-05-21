#[macro_use]
extern crate criterion;

use nexedit::Application;
use criterion::Criterion;
use std::path::PathBuf;

fn buffer_rendering(c: &mut Criterion) {
    let mut app = Application::new(&Vec::new()).unwrap();
    app.workspace
        .open_buffer(&PathBuf::from("src/commands/buffer.rs"))
        .unwrap();
    app.view
        .initialize_buffer(app.workspace.current_buffer.as_mut().unwrap())
        .unwrap();
    let buffer_data = app.workspace.current_buffer.as_ref().unwrap().data();

    c.bench_function("buffer rendering", move |b| {
        b.iter(|| {
            let mut presenter = app.view.build_presenter().unwrap();

            presenter
                .print_buffer(
                    app.workspace.current_buffer.as_ref().unwrap(),
                    &buffer_data,
                    &app.workspace.syntax_set,
                    None,
                    None,
                )
                .unwrap()
        })
    });
}

fn scrolled_buffer_rendering(c: &mut Criterion) {
    let mut app = Application::new(&Vec::new()).unwrap();
    app.workspace
        .open_buffer(&PathBuf::from("src/commands/buffer.rs"))
        .unwrap();
    app.view
        .initialize_buffer(app.workspace.current_buffer.as_mut().unwrap())
        .unwrap();
    let buffer_data = app.workspace.current_buffer.as_ref().unwrap().data();

    app.workspace
        .current_buffer
        .as_mut()
        .unwrap()
        .cursor
        .move_to_last_line();
    app.view
        .scroll_to_cursor(app.workspace.current_buffer.as_ref().unwrap())
        .unwrap();

    c.bench_function("scrolled buffer rendering", move |b| {
        b.iter(|| {
            let mut presenter = app.view.build_presenter().unwrap();

            presenter
                .print_buffer(
                    app.workspace.current_buffer.as_ref().unwrap(),
                    &buffer_data,
                    &app.workspace.syntax_set,
                    None,
                    None,
                )
                .unwrap()
        })
    });
}

criterion_group!(benches, buffer_rendering, scrolled_buffer_rendering);
criterion_main!(benches);
