use nexedit::Application;
use nexedit::Error;
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();

    if let Some(e) = Application::new(&args).and_then(|mut app| app.run()).err() {
        handle_error(&e)
    }
}

fn handle_error(error: &Error) {
    eprintln!("error: {}", error);

    for e in error.iter().skip(1) {
        eprintln!("caused by: {}", e);
    }

    if let Some(backtrace) = error.backtrace() {
        eprintln!("backtrace: {:?}", backtrace);
    }

    ::std::process::exit(1);
}
